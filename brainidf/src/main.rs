pub mod network_interfaces;
pub mod proto;

use std::{
    io::Write,
    net::{Ipv4Addr, UdpSocket},
};

use async_io::Async;
use embassy_executor::{Executor, Spawner};
use embassy_time::{Delay, Duration, Timer};

use embedded_hal_async::delay::DelayNs as _;
use esp_idf_svc::{
    eth::{AsyncEth, EspEth, EthDriver, RmiiEth},
    eventloop::EspSystemEventLoop,
    hal::{gpio, prelude::Peripherals},
    nvs::EspDefaultNvsPartition,
    sys::{esp_mac_type_t_ESP_MAC_WIFI_STA, esp_read_mac},
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, AuthMethod, ClientConfiguration, Configuration, EspWifi},
};
use log::info;
use smart_leds::RGB8;
use static_cell::StaticCell;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use crate::proto::Header;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const BRAIN_PORT: u16 = 8003;
const PINKY_PORT: u16 = 8002;

const LED_CH1_GPIO: u8 = 32;
const LED_CH2_GPIO: u8 = 2;

#[embassy_executor::task]
async fn run(spawner: Spawner) {
    // spawner.spawn(keep_wifi_connected()).unwrap();
    // loop {
    //     log::info!("Hello, world!");
    //     Timer::after(Duration::from_secs(1)).await;
    // }
}
static EXECUTOR: StaticCell<Executor> = StaticCell::new();

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _mounted_eventfs = esp_idf_svc::io::vfs::MountedEventfs::mount(5).unwrap();

    inner_main();
}

fn inner_main() {
    let exec = EXECUTOR.init(Executor::new());

    let _ = exec.run(|spawner| {
        // spawner.spawn(run(spawner)).unwrap();
        spawner.spawn(keep_wifi_connected()).unwrap();
    });
}

#[embassy_executor::task]
async fn keep_wifi_connected() {
    //  TODO: lift this even higher so we can give some pins to other tasks.
    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let timer_service = EspTaskTimerService::new().unwrap();

    let led_pin = peripherals.pins.gpio32;
    let channel = peripherals.rmt.channel0;
    let mut ws2812 = Ws2812Esp32Rmt::new(channel, led_pin).unwrap();
    let mut leds = vec![smart_leds::RGB8::new(0, 0, 0); 2048];
    ws2812.write_nocopy(leds.iter().cloned()).unwrap();

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

    let mut msg_id = 0i16;
    loop {
        // Connect logic takes temporary ownership and passes it back.
        let network_if = connect_network(network_if).await.unwrap();
        let bcast_addr = network_if.get_broadcast();

        info!("broadcast addr: {bcast_addr:?}");

        let mac = &mut [0u8; 6];

        // Current firmware always uses wifi station mac instead of ethernet mac for brain ID.
        unsafe { esp_read_mac(mac.as_mut_ptr(), esp_mac_type_t_ESP_MAC_WIFI_STA) };

        let brain_id = format!("{:02X}{:02X}{:02X}", mac[3], mac[4], mac[5]);

        let mut msg = Vec::with_capacity(128);
        proto::write_hello_msg(&mut msg, &brain_id);

        let header = Header::from_payload(msg_id, &msg);
        let mut msg_with_header = Vec::with_capacity(128);
        msg_with_header.extend_from_slice(&header.to_bytes());
        msg_with_header.extend(msg);

        msg_id = msg_id.wrapping_add_unsigned(1);

        info!("hello_msg {:x?}", &msg_with_header);

        let udp_sock = Async::<UdpSocket>::bind(([0, 0, 0, 0], BRAIN_PORT)).unwrap();

        udp_sock
            .send_to(&msg_with_header, (bcast_addr, PINKY_PORT))
            .await
            .unwrap();

        let rx_buf = &mut [0u8; 4096];

        // The current brain firmware only uses data CH1, though it has a CH2 as well.

        loop {
            if let Ok((count, from)) = udp_sock.recv_from(rx_buf).await {
                let mut rx_packet = &rx_buf[..count];
                let header = Header::from_reader(&mut rx_packet);
                if header.frame_offset == 0 {
                    // parse message
                    // TODO: handle messages longer than one packet
                    // Take care of R|G|B spanning the frame boundary.
                    let msg_type = rx_packet[0];
                    rx_packet = &rx_packet[1..];
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
                        rx_packet = &rx_packet[pong_len as usize..];
                    }
                    let desc_len = i32::from_be_bytes(rx_packet[..4].try_into().unwrap());
                    rx_packet = &rx_packet[4..];
                    let desc = &rx_packet[..desc_len as usize];
                    rx_packet = &rx_packet[desc_len as usize..];

                    let mut pixel_count = u16::from_be_bytes(rx_packet[..2].try_into().unwrap());
                    info!("pixel_count: {pixel_count}");
                    rx_packet = &rx_packet[2..];

                    // TODO: handle GRB as well
                    let start = std::time::Instant::now();
                    if desc == &[1, 1] {
                        for (i, rgb) in rx_packet.chunks_exact(3).enumerate() {
                            let [r, g, b] = rgb.try_into().unwrap();
                            if i < leds.len() {
                                leds[i] = RGB8::new(r, g, b);
                            }
                        }
                        //TODO: handle remainder
                    }
                    let write_leds = &leds.as_slice()[0..(leds.len().min(pixel_count as usize))];
                    ws2812.write_nocopy(write_leds.iter().cloned()).unwrap();
                    info!("write time {:?}", start.elapsed());
                }
                // info!("rx {count} bytes from {from:?}");
                // info!("{:x?}", &rx_buf[..count]);
            }
        }

        while network_if.is_up() {
            embassy_time::Delay.delay_ms(1000).await;
        }
    }
}

// struct

// enum ExtractedMessage<'a> {
//     ShadePanel { num_leds: u16, rgb_data: &'a [u8] },
// }

// impl<'a> ExtractedMessage<'a> {
//     fn parse(packet: &[u8]) -> Self {

//     }
// }

async fn connect_network(
    mut eth: AsyncEth<EspEth<'static, RmiiEth>>,
) -> anyhow::Result<impl NetworkInterface + 'static> {
    eth.start().await?;
    info!("Eth started");

    eth.wait_connected().await?;
    info!("Eth connected");
    eth.wait_netif_up().await?;

    info!("Eth netif_up");

    return Ok(eth);

    // {
    //     let nvs = EspDefaultNvsPartition::take()?;
    //     let mut wifi = AsyncWifi::wrap(
    //         EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
    //         sys_loop,
    //         timer_service,
    //     )?;

    //     let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
    //         ssid: SSID.try_into().unwrap(),
    //         bssid: None,
    //         auth_method: AuthMethod::WPA2Personal,
    //         password: PASSWORD.try_into().unwrap(),
    //         channel: None,
    //         ..Default::default()
    //     });

    //     wifi.set_configuration(&wifi_configuration)?;

    //     wifi.start().await?;
    //     info!("Wifi started");

    //     wifi.connect().await?;
    //     info!("Wifi connected");

    //     wifi.wait_netif_up().await?;
    //     info!("Wifi netif up");

    //     Ok(wifi)
    // }
}

trait NetworkInterface {
    fn get_ip(&self) -> Ipv4Addr;
    fn get_subnet(&self) -> Ipv4Addr;
    fn get_broadcast(&self) -> Ipv4Addr {
        let bcast_addr = Ipv4Addr::from(self.get_ip().to_bits() | (!self.get_subnet()).to_bits());
        bcast_addr
    }
    fn is_up(&self) -> bool;
}

impl<'a> NetworkInterface for AsyncWifi<EspWifi<'a>> {
    fn get_ip(&self) -> Ipv4Addr {
        self.wifi().sta_netif().get_ip_info().unwrap().ip
    }

    fn get_subnet(&self) -> Ipv4Addr {
        self.wifi()
            .sta_netif()
            .get_ip_info()
            .unwrap()
            .subnet
            .mask
            .into()
    }

    fn is_up(&self) -> bool {
        self.wifi().is_up().unwrap()
    }
}

impl<'a> NetworkInterface for AsyncEth<EspEth<'a, RmiiEth>> {
    fn get_ip(&self) -> Ipv4Addr {
        self.eth().netif().get_ip_info().unwrap().ip
    }

    fn get_subnet(&self) -> Ipv4Addr {
        info!(
            "subnet mask: {}",
            self.eth().netif().get_ip_info().unwrap().subnet.mask.0
        );
        self.eth().netif().get_ip_info().unwrap().subnet.mask.into()
    }

    fn is_up(&self) -> bool {
        self.eth().is_up().unwrap()
    }
}
