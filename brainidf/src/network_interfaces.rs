use esp_idf_svc::hal::gpio::{
    Gpio15, Gpio17, Gpio18, Gpio19, Gpio21, Gpio22, Gpio23, Gpio25, Gpio26, Gpio27,
};

use crate::*;

// Consume pins individually so we do not need the whole `Peripherals` struct.
pub fn setup_eth_driver(
    mac: esp_idf_svc::hal::mac::MAC,
    rdx0: Gpio25,
    rdx1: Gpio26,
    crs_dv: Gpio27,
    mdc: Gpio23,
    txd1: Gpio22,
    tx_en: Gpio21,
    txd0: Gpio19,
    mdio: Gpio18,
    ref_clk_config: Gpio17,
    rst: Option<Gpio15>,
    sys_loop: &EspSystemEventLoop,
    timer_service: &EspTaskTimerService,
) -> AsyncEth<EspEth<'static, RmiiEth>> {
    // Make sure to configure ethernet in sdkconfig and adjust the parameters below for your hardware
    let eth_driver = EthDriver::new_rmii(
            mac,
            rdx0,
            rdx1,
            crs_dv,
            mdc,
            txd1,
            tx_en,
            txd0,
            mdio,
            esp_idf_svc::eth::RmiiClockConfig::<gpio::Gpio0, gpio::Gpio16, gpio::Gpio17>::OutputInvertedGpio17(
                ref_clk_config,
            ),
            rst,
            esp_idf_svc::eth::RmiiEthChipset::LAN87XX,
            Some(0),
            sys_loop.clone(),
        ).unwrap();
    let eth = AsyncEth::wrap(
        EspEth::wrap(eth_driver).unwrap(),
        sys_loop.clone(),
        timer_service.clone(),
    )
    .unwrap();
    eth
}
