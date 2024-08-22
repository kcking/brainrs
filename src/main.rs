//! Embassy ESP-NOW Example
//!
//! Broadcasts, receives and sends messages via esp-now in an async way
//!
//! Because of the huge task-arena size configured this won't work on ESP32-S2

//% FEATURES: async embassy embassy-generic-timers esp-wifi esp-wifi/async esp-wifi/embassy-net esp-wifi/wifi-default esp-wifi/wifi esp-wifi/utils esp-wifi/esp-now
//% CHIPS: esp32 esp32s3 esp32c2 esp32c3 esp32c6

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_closure)]

use core::borrow::BorrowMut;

use embassy_executor::Spawner;
use embassy_futures::select::{select, select3, Either, Either3};
use embassy_time::{Duration, Instant, Ticker, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    dma::{Dma, DmaPriority},
    dma_buffers, dma_descriptors,
    gpio::{any_pin::AnyPin, GpioPin, Io, Level, Output},
    peripherals::Peripherals,
    prelude::*,
    rmt::Rmt,
    rng::Rng,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::{ErasedTimer, OneShotTimer, PeriodicTimer},
};
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};
use esp_println::println;
use esp_wifi::{
    esp_now::{PeerInfo, BROADCAST_ADDRESS},
    initialize, EspWifiInitFor,
};
use smart_leds::{
    colors,
    hsv::{hsv2rgb, Hsv},
    RGB, RGB8,
};
use static_cell::make_static;

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.spi2channel;
    let (_tx_buffer, tx_descriptors, _rx_buffer, rx_descriptors) = dma_buffers!(32000);

    use esp_hal::spi::master::prelude::*;
    let spi = Spi::new(peripherals.SPI2, 3u32.MHz(), SpiMode::Mode0, &clocks)
        .with_mosi(io.pins.gpio27)
        .with_dma(
            dma_channel.configure_for_async(false, DmaPriority::Priority0),
            tx_descriptors,
            rx_descriptors,
        );

    const N_LEDS: usize = 100;
    let mut ws = ws2812_async::Ws2812::<_, { 12 * N_LEDS }>::new(spi);

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
    let mut ticker2 = Ticker::every(Duration::from_secs(5));
    let mut led_ticker = Ticker::every(Duration::from_millis(33));
    let mut led_colors = [Hsv {
        hue: 0,
        val: 255,
        sat: 255,
    }; N_LEDS];
    let mut rbg_leds = [RGB8::default(); N_LEDS];
    let mut last_rx: Option<Instant> = None;
    loop {
        let res = select3(
            ticker.next(),
            async {
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
            },
            led_ticker.next(),
        )
        .await;

        match res {
            Either3::First(_) => {
                let status = esp_now.send_async(&BROADCAST_ADDRESS, b"0123456789").await;
            }
            Either3::Second(_) => {
                last_rx = Some(Instant::now());
            }
            Either3::Third(_) => {
                let val = if let Some(last_rx) = last_rx {
                    let secs_since = last_rx.elapsed().as_millis() as f32 / 1000f32;
                    (255f32 * (1f32 - secs_since).max(0f32)) as u8
                } else {
                    255
                };
                for (idx, c) in led_colors.iter_mut().enumerate() {
                    c.hue = c.hue.wrapping_add(1);
                    c.val = val;
                    rbg_leds[idx] = hsv2rgb(*c);
                }
                ws.write(rbg_leds.clone().into_iter()).await.unwrap();
            }
        }
    }
}
