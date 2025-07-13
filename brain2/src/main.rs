//! Embassy DHCP Example
//!
//!
//! Set SSID and PASSWORD env variable before running this example.
//!
//! This gets an ip address via DHCP then performs an HTTP get request to some
//! "random" server
//!
//! Because of the huge task-arena size configured this won't work on ESP32-S2

//% FEATURES: embassy esp-wifi esp-wifi/wifi esp-hal/unstable
//% CHIPS: esp32 esp32s2 esp32s3 esp32c2 esp32c3 esp32c6

#![no_std]
#![no_main]

pub mod proto;

use core::net::Ipv4Addr;

extern crate alloc;

use alloc::format;
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_net::{Runner, Stack, StackResources, tcp::TcpSocket, udp::PacketMetadata};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use embedded_hal_async::delay::{self, DelayNs};
use embedded_io::Write as _;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, rng::Rng, timer::timg::TimerGroup};
use esp_println::println;
use esp_wifi::{
    EspWifiController, init,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiState,
        sta_mac,
    },
};
use log::info;

use heapless::Vec;

use crate::proto::{Header, VecWriter};

esp_bootloader_esp_idf::esp_app_desc!();

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let mut rng = Rng::new(peripherals.RNG);

    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        init(timg0.timer0, rng.clone()).unwrap()
    );

    let (controller, interfaces) = esp_wifi::wifi::new(&esp_wifi_ctrl, peripherals.WIFI).unwrap();

    let wifi_interface = interfaces.sta;

    cfg_if::cfg_if! {
        if #[cfg(feature = "esp32")] {
            let timg1 = TimerGroup::new(peripherals.TIMG1);
            esp_hal_embassy::init(timg1.timer0);
        } else {
            use esp_hal::timer::systimer::SystemTimer;
            let systimer = SystemTimer::new(peripherals.SYSTIMER);
            esp_hal_embassy::init(systimer.alarm0);
        }
    }

    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();
    spawner.spawn(udp_task(stack.clone(), todo!())).ok();

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    loop {
        Timer::after(Duration::from_millis(1_000)).await;

        // let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        // socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        // let remote_endpoint = (Ipv4Addr::new(142, 250, 185, 115), 80);
        // println!("connecting...");
        // let r = socket.connect(remote_endpoint).await;
        // if let Err(e) = r {
        //     println!("connect error: {:?}", e);
        //     continue;
        // }
        // println!("connected!");
        // let mut buf = [0; 1024];
        // loop {
        //     use embedded_io_async::Write;
        //     let r = socket
        //         .write_all(b"GET / HTTP/1.0\r\nHost: www.mobile-j.de\r\n\r\n")
        //         .await;
        //     if let Err(e) = r {
        //         println!("write error: {:?}", e);
        //         break;
        //     }
        //     let n = match socket.read(&mut buf).await {
        //         Ok(0) => {
        //             println!("read EOF");
        //             break;
        //         }
        //         Ok(n) => n,
        //         Err(e) => {
        //             println!("read error: {:?}", e);
        //             break;
        //         }
        //     };
        //     println!("{}", core::str::from_utf8(&buf[..n]).unwrap());
        // }
        // Timer::after(Duration::from_millis(3000)).await;
    }
}

#[embassy_executor::task]
async fn udp_task(
    stack: Stack<'static>,
    udp_send_rx: embassy_sync::channel::Receiver<'static, NoopRawMutex, (), 16>,
) {
    let mut my_sta_mac = [0u8; 6];
    sta_mac(&mut my_sta_mac);
    let mac = my_sta_mac;
    let brain_id = format!("{:02X}{:02X}{:02X}", mac[3], mac[4], mac[5]);
    info!("brain_id = {brain_id}");

    //  TODO: handle connection lifecycle / retry on network error
    //  Default broadcast address
    let mut broadcast: embassy_net::IpEndpoint = "255.255.255.255:8002".parse().unwrap();
    stack.wait_config_up().await;
    if let Some(cfg) = stack.config_v4() {
        info!("cfg: {cfg:?}");
        //  dhcp-provided broadcast address
        broadcast = (cfg.address.broadcast().unwrap(), 8002).into();
        info!("{broadcast:?}");
    }

    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 4096];
    let mut socket = embassy_net::udp::UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(8003).unwrap();

    let mut w = VecWriter { buffer: Vec::new() };

    proto::write_hello_msg(&mut w, &brain_id);

    let mut msg_id = 0i16;
    let header = Header::from_payload(msg_id, w.buffer.as_slice());
    msg_id += 1;
    let mut msg_with_header = VecWriter {
        buffer: Vec::<u8, 128>::new(),
    };
    msg_with_header
        .write_all(header.to_bytes().as_slice())
        .unwrap();
    msg_with_header.write_all(w.buffer.as_slice()).unwrap();
    info!("hello_msg {:x?}", &msg_with_header.buffer);

    socket
        .send_to(&msg_with_header.buffer, broadcast)
        .await
        .unwrap();

    let mut rx_buf = [0; 4096];

    loop {
        select(socket.recv_from(&mut rx_buf), udp_send_rx.receive());
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.into(),
                password: PASSWORD.into(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");

            println!("Scan");
            let result = controller.scan_n_async(10).await.unwrap();
            for ap in result {
                println!("{:?}", ap);
            }
        }
        println!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
