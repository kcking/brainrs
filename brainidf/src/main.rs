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
use static_cell::StaticCell;

use crate::proto::Header;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const BRAIN_PORT: u16 = 8003;
const PINKY_PORT: u16 = 8002;

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
    let mut msg_id = 0i16;
    loop {
        let network_if = connect_network().await.unwrap();
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

        let rx_buf = &mut [0u8; 32];
        loop {
            if let Ok((count, from)) = udp_sock.recv_from(rx_buf).await {
                // info!("rx {count} bytes from {from:?}");
                // info!("{:x?}", &rx_buf[..count]);
            }
        }

        while network_if.is_up() {
            embassy_time::Delay.delay_ms(1000).await;
        }
    }
}

async fn connect_network() -> anyhow::Result<impl NetworkInterface + 'static> {
    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;
    let sys_loop = EspSystemEventLoop::take()?;
    let timer_service = EspTaskTimerService::new()?;

    {
        // Make sure to configure ethernet in sdkconfig and adjust the parameters below for your hardware
        let eth_driver = EthDriver::new_rmii(
            peripherals.mac,
            pins.gpio25,
            pins.gpio26,
            pins.gpio27,
            pins.gpio23,
            pins.gpio22,
            pins.gpio21,
            pins.gpio19,
            pins.gpio18,
            esp_idf_svc::eth::RmiiClockConfig::<gpio::Gpio0, gpio::Gpio16, gpio::Gpio17>::OutputInvertedGpio17(
                pins.gpio17,
            ),
            Some(pins.gpio15),
            esp_idf_svc::eth::RmiiEthChipset::LAN87XX,
            Some(0),
            sys_loop.clone(),
        )?;
        let mut eth = AsyncEth::wrap(
            EspEth::wrap(eth_driver)?,
            sys_loop.clone(),
            timer_service.clone(),
        )?;

        eth.start().await?;
        info!("Eth started");

        eth.wait_connected().await?;
        info!("Eth connected");
        eth.wait_netif_up().await?;

        info!("Eth netif_up");

        return Ok(eth);
    }

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
