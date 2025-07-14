use esp_idf_svc::hal::gpio::{
    Gpio15, Gpio17, Gpio18, Gpio19, Gpio21, Gpio22, Gpio23, Gpio25, Gpio26, Gpio27,
};

use crate::*;

pub async fn connect_network(
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

pub trait NetworkInterface {
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
