#![allow(unused)]
pub mod dithering;
pub mod network_interfaces;
pub mod proto;

use std::{
    io::Write,
    net::{Ipv4Addr, UdpSocket},
    sync::Mutex,
    time::Instant,
};

use smart_leds::SmartLedsWrite;

use async_io::Async;
use embassy_executor::{Executor, Spawner};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Delay, Duration, Timer};

use embedded_hal_async::delay::DelayNs as _;
use esp_idf_svc::hal::task::watchdog::{TWDTConfig, TWDTDriver};
use esp_idf_svc::{
    eth::{AsyncEth, EspEth, EthDriver, RmiiEth},
    eventloop::EspSystemEventLoop,
    hal::{
        cpu::Core,
        gpio::{self, Gpio0, OutputPin},
        peripheral::Peripheral,
        prelude::Peripherals,
        rmt::RmtChannel,
        spi::{Dma, Spi, SpiAnyPins, SpiBusDriver, SpiConfig, SpiDriver, SpiDriverConfig},
        task::{block_on, thread::ThreadSpawnConfiguration, watchdog::TWDT, yield_now},
        units::Hertz,
    },
    nvs::EspDefaultNvsPartition,
    sys::{ESP_TASK_PRIO_MAX, esp_mac_type_t_ESP_MAC_WIFI_STA, esp_read_mac},
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, AuthMethod, ClientConfiguration, Configuration, EspWifi},
};
use log::{error, info, trace};
use rgb::AsPixels;
use smart_leds::RGB8;
use static_cell::StaticCell;

use crate::{
    network_interfaces::{NetworkInterface, connect_eth},
    proto::{
        BrainHello, FRAGMENT_MAX, Header, MessageType, PONG_DATA_MAX, Ping, create_hello_msg,
        prepend_header, prepend_header_heapless,
    },
};

#[cfg(feature = "wifi")]
const SSID: &str = env!("SSID");
#[cfg(feature = "wifi")]
const PASSWORD: &str = env!("PASSWORD");
const BRAIN_PORT: u16 = 8003;
const PINKY_PORT: u16 = 8002;
const MAX_LEDS: usize = 2048;

// The current brain firmware only uses data CH1, though it has a CH2 as well.
const LED_CH1_GPIO: u8 = 32;
const LED_CH2_GPIO: u8 = 2;

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

static LED_MUTEX: Mutex<Vec<RGB8>> = Mutex::new(Vec::new());

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _mounted_eventfs = esp_idf_svc::io::vfs::MountedEventfs::mount(5).unwrap();

    ThreadSpawnConfiguration {
        pin_to_core: Some(Core::Core0),
        // stack_size: 32000,
        ..ThreadSpawnConfiguration::default()
    }
    .set();
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
    spi: impl Peripheral<P = impl SpiAnyPins>,
    task_watchdog_timer: impl Peripheral<P = TWDT>,
) {
    // SPI/DMA Config
    let config =
        SpiDriverConfig::new().dma(esp_idf_svc::hal::spi::Dma::Channel1(MAX_LEDS * 3 + 64));
    let spi_driver = SpiDriver::new_without_sclk(
        spi,
        data_gpio,
        None::<esp_idf_svc::hal::gpio::Gpio0>,
        &config,
    )
    .unwrap();
    let spi_config = SpiConfig::new().write_only(true).baudrate(Hertz(3_000_000));
    let spi_driver = SpiBusDriver::new(spi_driver, &spi_config).unwrap();
    let max_framerate = 60;
    let mut frame_number = 0u64;
    let mut dma_buf = vec![0u8; MAX_LEDS * 12 + 120];
    let mut ws_driver = ws2812_spi::prerendered::Ws2812::new(spi_driver, &mut dma_buf);

    let mut frame_ticker = embassy_time::Ticker::every(Duration::from_hz(max_framerate));
    let mut leds = vec![];

    loop {
        block_on(frame_ticker.next());

        {
            let data = LED_MUTEX.lock().unwrap();
            leds.resize(data.len(), Default::default());
            leds.copy_from_slice(data.as_slice());
        }
        trace!("got led frame");
        let dithered = leds.iter().enumerate().map(|(pixel_idx, rgb)| {
            // FLIP R and G for the spi driver
            RGB8::new(
                dithering::correct_22(rgb.g, frame_number as u32, pixel_idx as u32),
                dithering::correct_22(rgb.r, frame_number as u32, pixel_idx as u32),
                dithering::correct_22(rgb.b, frame_number as u32, pixel_idx as u32),
            )
        });
        ws_driver.write(dithered).unwrap();

        frame_number += 1;
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

    ThreadSpawnConfiguration {
        pin_to_core: Some(Core::Core1),
        priority: ESP_TASK_PRIO_MAX as u8 - 1,
        ..ThreadSpawnConfiguration::default()
    }
    .set();
    std::thread::spawn(move || {
        led_write_task(led_pin, channel, peripherals.spi2, peripherals.twdt)
    });
    ThreadSpawnConfiguration::default().set();
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
        let mut next_pong_data = None;

        loop {
            let udp_rx_with_timeout =
                embassy_time::with_timeout(pinky_liveness_ttl, udp_sock.recv_from(rx_buf));
            match udp_rx_with_timeout.await {
                Ok(Ok((count, from))) => {
                    let mut rx_packet = &rx_buf[..count];
                    let header = Header::from_reader(&mut rx_packet);
                    msg_id = header.id.wrapping_add_unsigned(1);
                    let res = led_state.on_message(header, rx_packet);
                    if let Some(pong_data) = res.pong_data {
                        info!("save pong data");
                        next_pong_data = Some(pong_data);
                    }
                    match res.action {
                        OnMessageAction::Nothing => {}
                        OnMessageAction::WriteLeds => {
                            let leds = led_state.get_leds();
                            trace!("sent led frame");
                            {
                                let mut locked_leds = LED_MUTEX.lock().unwrap();
                                locked_leds.clear();
                                locked_leds.extend_from_slice(&leds);
                            }

                            if let Some(next_pong_data) = next_pong_data.take() {
                                info!("sending pong");
                                let msg = Ping {
                                    data: next_pong_data,
                                    is_pong: true,
                                };
                                let Ok(msg_heapless) =
                                    heapless::Vec::<u8, FRAGMENT_MAX>::from_slice(
                                        &msg.to_heapless(),
                                    )
                                else {
                                    error!("message copy failed");
                                    continue;
                                };
                                let msg = prepend_header_heapless(msg_id, msg_heapless);
                                msg_id = msg_id.wrapping_add_unsigned(1);
                                udp_sock.send_to(&msg, from).await;
                            }
                        }
                        OnMessageAction::SendBrainHello => {
                            let msg = create_hello_msg(msg_id, &brain_id);
                            msg_id = msg_id.wrapping_add_unsigned(1);
                            // TODO: log error
                            // NOTE: broadcast didn't work here
                            let _ = udp_sock.send_to(&msg, from).await;
                        }
                        OnMessageAction::DownloadFirmware(url) => {
                            info!("<- Download Firmware {url}");
                        }
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

struct OnMessageResult {
    pub pong_data: Option<heapless::Vec<u8, PONG_DATA_MAX>>,
    pub action: OnMessageAction,
}
enum OnMessageAction {
    Nothing,
    WriteLeds,
    SendBrainHello,
    DownloadFirmware(heapless::String<512>),
}

/// State machine to handle message unframing. Maintains the current state of
/// all LEDs with no additional memory overhead.
struct LedState {
    last_header: Option<Header>,
    last_led_byte_idx: Option<usize>,
    pixel_count: Option<usize>,
    leds: Vec<u8>,
    palette: Option<Vec<u8>>,
}

impl LedState {
    pub fn new(n_leds: usize) -> Self {
        Self {
            last_header: None,
            last_led_byte_idx: None,
            leds: vec![0u8; n_leds * 3],
            pixel_count: None,
            palette: None,
        }
    }
    pub fn get_leds(&self) -> &[RGB8] {
        let pixels = self.leds.as_pixels();
        if let Some(pixel_count) = self.pixel_count {
            &pixels[..pixel_count.min(pixels.len())]
        } else {
            pixels
        }
    }
    // Returns: whether caller should write LED data out to RMT
    pub fn on_message(&mut self, header: Header, mut rx_packet: &[u8]) -> OnMessageResult {
        let mut pong_data = None;

        if header.frame_offset == 0 {
            // Reset framing state.
            self.reset();

            let msg_type = rx_packet[0];
            rx_packet = &rx_packet[1..];
            if msg_type == MessageType::BrainIdRequest as u8 {
                return OnMessageResult {
                    pong_data: pong_data,
                    action: OnMessageAction::SendBrainHello,
                };
            }
            if msg_type == MessageType::UseFirmware as u8 {
                match proto::UseFirmware::parse(rx_packet) {
                    Ok(use_firmware) => {
                        return OnMessageResult {
                            action: OnMessageAction::DownloadFirmware(use_firmware.url),
                            pong_data: None,
                        };
                    }
                    Err(e) => {
                        error!("Failed to parse UseFirmware message {e:?}");
                        return OnMessageResult {
                            action: OnMessageAction::Nothing,
                            pong_data: None,
                        };
                    }
                }
            }
            if msg_type != MessageType::BrainPanelShade as u8 {
                info!("got unsupported message type {msg_type}");
                return OnMessageResult {
                    pong_data: pong_data,
                    action: OnMessageAction::Nothing,
                };
            }

            /*
             * Brain Panel Shade message format:
             * 12byte header | 0x01 (message type) | 1byte bool hasPongData | optional bytearray (int + bytes) |
             * | bytearray shader descrption  (2-bytes: 0x01 (PIXEL type),  0x01 (encoding RGB))
             */
            let has_pong = rx_packet[0] > 0;
            rx_packet = &rx_packet[1..];
            if has_pong {
                let pong_len = i32::from_be_bytes(rx_packet[..4].try_into().unwrap());
                rx_packet = &rx_packet[4..];
                info!("got pong data");
                if let Ok(pong_data_buf) =
                    heapless::Vec::from_slice(&rx_packet[..pong_len as usize])
                {
                    pong_data = Some(pong_data_buf);
                } else {
                    error!("Pong data of length {pong_len} too long, max is {PONG_DATA_MAX}");
                }
                rx_packet = &rx_packet[pong_len as usize..];
            }
            let desc_len = i32::from_be_bytes(rx_packet[..4].try_into().unwrap());
            rx_packet = &rx_packet[4..];
            let desc = &rx_packet[..desc_len as usize];
            rx_packet = &rx_packet[desc_len as usize..];

            let pixel_count = u16::from_be_bytes(rx_packet[..2].try_into().unwrap());
            rx_packet = &rx_packet[2..];
            self.pixel_count = Some(pixel_count as usize);

            match desc {
                &[1, 2] => {
                    // ARGB, but we ignore A
                    let palette_len = 2 * 4;
                    // Indexed palette of 2 colors
                    let palette = rx_packet[..palette_len].to_vec();
                    rx_packet = &rx_packet[palette_len..];
                    self.palette = Some(palette);
                }
                &[1, 1] => {
                    self.palette = None;
                }
                _ => {
                    //TODO: support mapping descriptor [1, 2]
                    info!("unsupported descriptor {:x?}", desc);
                    // Only pixel shader currently supported
                    self.reset();
                    return OnMessageResult {
                        pong_data: pong_data,
                        action: OnMessageAction::Nothing,
                    };
                }
            }
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
                log::debug!(
                    "noncontiguous frame received, resetting {:?} {:?}",
                    &self.last_header,
                    &header
                );
                self.reset();
                return OnMessageResult {
                    pong_data: pong_data,
                    action: OnMessageAction::Nothing,
                };
            }
        }

        let offset = self.last_led_byte_idx.unwrap_or(0);
        if let Some(ref palette) = self.palette {
            // TODO: handle out of bounds inputs (more pixels than we store)
            // NOTE: only 2 palette supported
            for (i, b) in rx_packet.iter().enumerate() {
                for bit in 0..8 {
                    let led_index = (offset + i) * 8 + bit;
                    if led_index * 3 >= self.leds.len() {
                        continue;
                    }
                    let pixel = &mut self.leds[led_index * 3..(led_index + 1) * 3];
                    if (1 << (8 - bit)) & b == 0 {
                        pixel.copy_from_slice(&palette[1..4]);
                    } else {
                        pixel.copy_from_slice(&palette[5..8]);
                    }
                }
            }

            self.last_led_byte_idx = Some(offset + rx_packet.len());
        } else {
            // dbg!(offset, rx_packet.len(), self.leds.len());
            let leds_to_copy = if offset + rx_packet.len() >= self.leds.len() {
                self.leds.len().saturating_sub(offset)
            } else {
                rx_packet.len()
            };
            self.leds.as_mut_slice()[offset..offset + leds_to_copy]
                .copy_from_slice(&rx_packet[..leds_to_copy]);

            self.last_led_byte_idx = Some(offset + leds_to_copy);
        }

        let ready_to_write = header.frame_offset + header.frame_size as i32 == header.msg_size;
        self.last_header = Some(header);
        OnMessageResult {
            pong_data,
            action: if ready_to_write {
                OnMessageAction::WriteLeds
            } else {
                OnMessageAction::Nothing
            },
        }
    }

    fn reset(&mut self) {
        self.last_header = None;
        self.last_led_byte_idx = None;
        self.palette = None;
    }
}
