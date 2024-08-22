//! Embassy ESP-NOW Example
//!
//! Broadcasts, receives and sends messages via esp-now in an async way
//!
//! Because of the huge task-arena size configured this won't work on ESP32-S2

//% FEATURES: async embassy embassy-generic-timers esp-wifi esp-wifi/async esp-wifi/embassy-net esp-wifi/wifi-default esp-wifi/wifi esp-wifi/utils esp-wifi/esp-now
//% CHIPS: esp32 esp32s3 esp32c2 esp32c3 esp32c6

#![no_std]
#![no_main]

use core::borrow::BorrowMut;

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    gpio::{any_pin::AnyPin, GpioPin, Io, Level, Output},
    peripherals::Peripherals,
    prelude::*,
    rmt::Rmt,
    rng::Rng,
    system::SystemControl,
    timer::{ErasedTimer, OneShotTimer, PeriodicTimer},
};
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};
use esp_println::println;
use esp_wifi::{
    esp_now::{PeerInfo, BROADCAST_ADDRESS},
    initialize, EspWifiInitFor,
};
use smart_leds::colors;

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[embassy_executor::task]
async fn blink(mut leds: SmartLedsAdapter<esp_hal::rmt::Channel<esp_hal::Blocking, 0>, 25>) {
    loop {
        use smart_leds::SmartLedsWrite as _;
        // THIS WORKS but colors are _basically_ white
        _ = leds.write([colors::BLACK]);
        Timer::after_millis(150).await;
        // _ = leds.write([colors::ROSY_BROWN]);
        Timer::after_millis(150).await;
    }
}

#[main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let rmt = Rmt::new(peripherals.RMT, 80.MHz(), &clocks, None).unwrap();

    let rmt_buffer = smartLedBuffer!(1);
    let led = SmartLedsAdapter::new(rmt.channel0, io.pins.gpio27, rmt_buffer, &clocks);
    spawner.must_spawn(blink(led));

    let timer = PeriodicTimer::new(
        esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0, &clocks, None)
            .timer0
            .into(),
    );

    let init = initialize(
        EspWifiInitFor::Wifi,
        timer,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
        &clocks,
    )
    .unwrap();

    let wifi = peripherals.WIFI;
    let mut esp_now = esp_wifi::esp_now::EspNow::new(&init, wifi).unwrap();
    println!("esp-now version {}", esp_now.get_version().unwrap());

    #[cfg(feature = "esp32")]
    {
        let timg1 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1, &clocks, None);
        esp_hal_embassy::init(
            &clocks,
            mk_static!(
                [OneShotTimer<ErasedTimer>; 1],
                [OneShotTimer::new(timg1.timer0.into())]
            ),
        );
    }

    #[cfg(not(feature = "esp32"))]
    {
        let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
        esp_hal_embassy::init(
            &clocks,
            mk_static!(
                [OneShotTimer<ErasedTimer>; 1],
                [OneShotTimer::new(systimer.alarm0.into())]
            ),
        );
    }

    let mut ticker = Ticker::every(Duration::from_secs(5));
    loop {
        let res = select(ticker.next(), async {
            let r = esp_now.receive_async().await;
            println!("Received {:?}", r);
            if r.info.dst_address == BROADCAST_ADDRESS {
                if !esp_now.peer_exists(&r.info.src_address) {
                    esp_now
                        .add_peer(PeerInfo {
                            peer_address: r.info.src_address,
                            lmk: None,
                            channel: None,
                            encrypt: false,
                        })
                        .unwrap();
                }
                let status = esp_now.send_async(&r.info.src_address, b"Hello Peer").await;
                println!("Send hello to peer status: {:?}", status);
            }
        })
        .await;

        match res {
            Either::First(_) => {
                println!("Send");
                let status = esp_now.send_async(&BROADCAST_ADDRESS, b"0123456789").await;
                println!("Send broadcast status: {:?}", status)
            }
            Either::Second(_) => (),
        }
    }
}
