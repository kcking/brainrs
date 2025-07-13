pub mod proto;

use std::{
    io::Write,
    net::{Ipv4Addr, UdpSocket},
};

use async_io::Async;
use embassy_executor::{Executor, Spawner};
use embassy_time::{Duration, Timer};

use embedded_hal_async::delay::DelayNs as _;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::prelude::Peripherals,
    nvs::EspDefaultNvsPartition,
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
    spawner.spawn(keep_wifi_connected()).unwrap();
    loop {
        log::info!("Hello, world!");
        Timer::after(Duration::from_secs(1)).await;
    }
}
static EXECUTOR: StaticCell<Executor> = StaticCell::new();

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _mounted_eventfs = esp_idf_svc::io::vfs::MountedEventfs::mount(5).unwrap();

    std::thread::Builder::new()
        .stack_size(60000)
        .spawn(inner_main)
        .unwrap()
        .join()
        .unwrap();
}

fn inner_main() {
    let exec = EXECUTOR.init(Executor::new());

    let _ = exec.run(|spawner| {
        spawner.spawn(run(spawner)).unwrap();
    });
    // spawner.spawn(keep_wifi_connected()).unwrap();
}

#[embassy_executor::task]
async fn keep_wifi_connected() {
    loop {
        let wifi = connect_wifi().await.unwrap();
        let ip_info = wifi.wifi().sta_netif().get_ip_info().unwrap();
        let bcast_addr =
            Ipv4Addr::from(ip_info.ip.to_bits() | (!Ipv4Addr::from(ip_info.subnet.mask).to_bits()));

        info!("broadcast addr: {bcast_addr:?}");

        let mac = wifi
            .wifi()
            .get_mac(esp_idf_svc::wifi::WifiDeviceId::Sta)
            .unwrap();

        let brain_id = format!("{:02X}{:02X}{:02X}", mac[3], mac[4], mac[5]);

        let mut msg = Vec::with_capacity(128);
        proto::write_hello_msg(&mut msg, &brain_id);

        let mut msg_id = 0i16;
        let header = Header::from_payload(msg_id, &msg);
        let mut msg_with_header = Vec::with_capacity(128);
        msg_with_header.extend_from_slice(&header.to_bytes());
        msg_with_header.extend(msg);

        msg_id += 1;

        info!("hello_msg {:x?}", &msg_with_header);

        let udp_sock = Async::<UdpSocket>::bind(([0, 0, 0, 0], BRAIN_PORT)).unwrap();

        udp_sock
            .send_to(&msg_with_header, (bcast_addr, PINKY_PORT))
            .await
            .unwrap();

        while wifi.is_up().unwrap_or(false) {
            embassy_time::Delay.delay_ms(1000).await;
        }
    }
}

async fn connect_wifi() -> anyhow::Result<AsyncWifi<EspWifi<'static>>> {
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let timer_service = EspTaskTimerService::new()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = AsyncWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
        timer_service,
    )?;

    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start().await?;
    info!("Wifi started");

    wifi.connect().await?;
    info!("Wifi connected");

    wifi.wait_netif_up().await?;
    info!("Wifi netif up");

    Ok(wifi)
}
