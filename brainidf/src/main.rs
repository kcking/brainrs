#![allow(unused)]
pub mod network_interfaces;
pub mod proto;

use std::{
    io::Write,
    net::{Ipv4Addr, UdpSocket},
    time::Instant,
};

use async_io::Async;
use embassy_executor::{Executor, Spawner};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Delay, Duration, Timer};

use embedded_hal_async::delay::DelayNs as _;
use esp_idf_svc::{
    eth::{AsyncEth, EspEth, EthDriver, RmiiEth},
    eventloop::EspSystemEventLoop,
    hal::{
        cpu::Core,
        gpio::{self, OutputPin},
        peripheral::Peripheral,
        prelude::Peripherals,
        rmt::RmtChannel,
        task::block_on,
    },
    nvs::EspDefaultNvsPartition,
    sys::{esp_mac_type_t_ESP_MAC_WIFI_STA, esp_read_mac},
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, AuthMethod, ClientConfiguration, Configuration, EspWifi},
};
use log::{error, info, trace};
use rgb::AsPixels;
use smart_leds::RGB8;
use static_cell::StaticCell;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use crate::{
    network_interfaces::{NetworkInterface, connect_eth},
    proto::{Header, MessageType, create_hello_msg},
};

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const BRAIN_PORT: u16 = 8003;
const PINKY_PORT: u16 = 8002;
const MAX_LEDS: usize = 2048;

// The current brain firmware only uses data CH1, though it has a CH2 as well.
const LED_CH1_GPIO: u8 = 32;
const LED_CH2_GPIO: u8 = 2;

static EXECUTOR: StaticCell<Executor> = StaticCell::new();
// A `Signal` ensures only one copy of the LEDs is in flight at a time.
static LED_FRAME_SIGNAL: Signal<CriticalSectionRawMutex, Vec<RGB8>> = Signal::new();

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _mounted_eventfs = esp_idf_svc::io::vfs::MountedEventfs::mount(5).unwrap();

    let exec = EXECUTOR.init(Executor::new());

    let _ = exec.run(|spawner| {
        spawner.spawn(main_task()).unwrap();
    });
}

// This task is blocking since esp-hal-idf doesn't support non-blocking writes
// to RMT.
// TODO: consider pinning this task to Core1
fn led_write_task(
    data_gpio: impl Peripheral<P = impl OutputPin>,
    rmt: impl Peripheral<P = impl RmtChannel>,
) {
    let mut ws2812 = Ws2812Esp32Rmt::new(rmt, data_gpio).unwrap();
    // reset to black at start
    ws2812
        .write_nocopy(vec![RGB8::new(0, 0, 0)].iter().cloned())
        .unwrap();
    loop {
        let data = block_on(LED_FRAME_SIGNAL.wait());
        trace!("got led frame");
        ws2812.write_nocopy(data).unwrap();
    }
}

#[embassy_executor::task]
async fn main_task() {
    //  TODO: lift this even higher so we can give some pins to other tasks.
    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let timer_service = EspTaskTimerService::new().unwrap();

    let led_pin = peripherals.pins.gpio32;
    let channel = peripherals.rmt.channel0;
    std::thread::spawn(move || led_write_task(led_pin, channel));
    let mut led_state = LedState::new(MAX_LEDS);

    #[cfg(feature = "ethernet")]
    let network_if = network_interfaces::setup_eth_driver(
        peripherals.mac,
        peripherals.pins.gpio25,
        peripherals.pins.gpio26,
        peripherals.pins.gpio27,
        peripherals.pins.gpio23,
        peripherals.pins.gpio22,
        peripherals.pins.gpio21,
        peripherals.pins.gpio19,
        peripherals.pins.gpio18,
        peripherals.pins.gpio17,
        Some(peripherals.pins.gpio15),
        &sys_loop,
        &timer_service,
    );
    #[cfg(feature = "wifi")]
    let network_if =
        network_interfaces::setup_wifi_driver(peripherals.modem, &sys_loop, &timer_service);

    let mut msg_id = 0i16;
    loop {
        // Connect logic takes temporary ownership and passes it back.
        // TODO: make outer_connect name better, runs connection logic, eth/wifi
        // agnostic.
        let network_if = network_if.outer_connect().await.unwrap();
        let bcast_addr = network_if.get_broadcast();

        info!("broadcast addr: {bcast_addr:?}");

        let mac = &mut [0u8; 6];

        // Current firmware always uses wifi station mac instead of ethernet mac for brain ID.
        unsafe { esp_read_mac(mac.as_mut_ptr(), esp_mac_type_t_ESP_MAC_WIFI_STA) };

        let brain_id = format!("{:02X}{:02X}{:02X}", mac[3], mac[4], mac[5]);

        let hello_msg = create_hello_msg(msg_id, &brain_id);
        msg_id = msg_id.wrapping_add_unsigned(1);

        info!("hello_msg {:x?}", &hello_msg);

        let udp_sock = Async::<UdpSocket>::bind(([0, 0, 0, 0], BRAIN_PORT)).unwrap();

        udp_sock
            .send_to(&hello_msg, (bcast_addr, PINKY_PORT))
            .await
            .unwrap();

        let rx_buf = &mut [0u8; 4096];

        let pinky_liveness_ttl = Duration::from_millis(5_000);

        loop {
            let udp_rx_with_timeout =
                embassy_time::with_timeout(pinky_liveness_ttl, udp_sock.recv_from(rx_buf));
            match udp_rx_with_timeout.await {
                Ok(Ok((count, _from))) => {
                    let mut rx_packet = &rx_buf[..count];
                    let header = Header::from_reader(&mut rx_packet);
                    if led_state.on_message(header, rx_packet) {
                        let leds = led_state.get_leds();
                        trace!("sent led frame");
                        LED_FRAME_SIGNAL.signal(leds);
                    }
                }
                Ok(Err(e)) => {
                    //  TODO: handle network rx error, maybe just panic?
                    error!("Unhandled network rx error {e:?}");
                }
                Err(_) => {
                    info!("Haven't heard from pinky in {pinky_liveness_ttl:?}, sending hello");
                    let hello_msg = create_hello_msg(msg_id, &brain_id);
                    msg_id = msg_id.wrapping_add_unsigned(1);
                    udp_sock
                        .send_to(&hello_msg, (bcast_addr, PINKY_PORT))
                        .await
                        .unwrap();
                }
            }
        }

        while NetworkInterface::is_up(&network_if) {
            embassy_time::Delay.delay_ms(1000).await;
        }
    }
}

/// State machine to handle message unframing. Maintains the current state of
/// all LEDs with no additional memory overhead.
struct LedState {
    last_header: Option<Header>,
    last_led_byte_idx: Option<usize>,
    pixel_count: Option<usize>,
    leds: Vec<u8>,
}

impl LedState {
    pub fn new(n_leds: usize) -> Self {
        Self {
            last_header: None,
            last_led_byte_idx: None,
            leds: vec![0u8; n_leds * 3],
            pixel_count: None,
        }
    }
    pub fn get_leds(&self) -> Vec<RGB8> {
        let pixels = self.leds.as_slice().as_pixels();
        if let Some(pixel_count) = self.pixel_count {
            &pixels[..pixel_count.min(pixels.len())]
        } else {
            pixels
        }
        .into()
    }
    // Returns: whether caller should write LED data out to RMT
    pub fn on_message(&mut self, header: Header, mut rx_packet: &[u8]) -> bool {
        if header.frame_offset == 0 {
            // Reset framing state.
            self.last_led_byte_idx = None;
            self.last_header = None;

            let msg_type = rx_packet[0];
            if msg_type != MessageType::BrainPanelShade as u8 {
                return false;
            }
            rx_packet = &rx_packet[1..];

            /*
             * Brain Panel Shade message format:
             * 12byte header | 0x01 (message type) | 1byte bool hasPongData | optional bytearray (int + bytes) |
             * | bytearray shader descrption  (2-bytes: 0x01 (PIXEL type),  0x01 (encoding RGB))
             */
            let has_pong = rx_packet[0] > 0;
            rx_packet = &rx_packet[1..];
            if has_pong {
                // Ignore pong data for now.
                let pong_len = i32::from_be_bytes(rx_packet[..4].try_into().unwrap());
                rx_packet = &rx_packet[4..];
                rx_packet = &rx_packet[pong_len as usize..];
            }
            let desc_len = i32::from_be_bytes(rx_packet[..4].try_into().unwrap());
            rx_packet = &rx_packet[4..];
            let desc = &rx_packet[..desc_len as usize];
            rx_packet = &rx_packet[desc_len as usize..];

            if desc != &[1, 1] {
                // Only pixel shader currently supported
                self.last_header = None;
                self.last_led_byte_idx = None;
                return false;
            }

            let pixel_count = u16::from_be_bytes(rx_packet[..2].try_into().unwrap());
            rx_packet = &rx_packet[2..];
            self.pixel_count = Some(pixel_count as usize);
        } else {
            if let Some(last_header) = &self.last_header
                && last_header.id == header.id
                && (last_header.frame_offset as usize) + (last_header.frame_size as usize)
                    == header.frame_offset as usize
            {
                // Direct continuation of last packet, fall through to writing LED data.
            } else {
                // Not a continuation. Regardless of whether this is a network
                // error or we actually finished the last message, reset state.
                self.last_led_byte_idx = None;
                self.last_header = None;
                return false;
            }
        }

        let offset = self.last_led_byte_idx.unwrap_or(0);
        self.leds.as_mut_slice()[offset..offset + rx_packet.len()].copy_from_slice(rx_packet);
        self.last_led_byte_idx = Some(offset + rx_packet.len());

        let ready_to_write = header.frame_offset + header.frame_size as i32 == header.msg_size;
        self.last_header = Some(header);
        return ready_to_write;
    }
}
