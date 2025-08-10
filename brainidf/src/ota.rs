use crate::*;

use anyhow::Context;
use esp_idf_svc::{
    http::{
        Method,
        client::{EspHttpConnection, Response},
    },
    ota::{EspFirmwareInfoLoad, EspOta, EspOtaUpdate, FirmwareInfo},
};

use embedded_svc::http::client::Client as HttpClient;

pub fn update_firmware<const N: usize>(url: &heapless::String<N>) -> anyhow::Result<()> {
    let mut client = HttpClient::wrap(EspHttpConnection::new(&Default::default())?);
    update_firmware_with_client(&mut client, &url)?;

    Ok(())
}

fn update_firmware_with_client<const N: usize>(
    client: &mut HttpClient<EspHttpConnection>,
    url: &heapless::String<N>,
) -> anyhow::Result<()> {
    let mut ota = EspOta::new().context("failed to obtain OTA instance")?;

    info!("Downloading update from {url}");

    let headers = [("Accept", "application/octet-stream")];
    let request = client
        .request(Method::Get, &url, &headers)
        .context("failed to create update request")?;
    let response = request.submit().context("failed to send update request")?;

    if response.status() == 200 {
        info!("updating...");
        let mut update = ota.initiate_update().context("failed to initiate update")?;

        match download_update(response, &mut update).context("failed to download update") {
            Ok(_) => {
                info!("Update done. Restarting...");
                update.complete().context("failed to complete update")?;
                esp_idf_svc::hal::reset::restart();
            }
            Err(err) => {
                error!("Update failed: {err}");
                update.abort().context("failed to abort update")?;
            }
        };
    } else {
        error!("Update failed, got status {}", response.status());
        anyhow::bail!("bad status from firmware download");
    }

    Ok(())
}

fn download_update(
    mut response: Response<&mut EspHttpConnection>,
    update: &mut EspOtaUpdate<'_>,
) -> anyhow::Result<()> {
    let mut buffer = [0_u8; 1024];

    // You can optionally read the firmware metadata header.
    // It contains information like version and signature you can check before continuing the update
    let update_info = read_firmware_info(&mut buffer, &mut response, update)?;
    info!("Update version: {}", update_info.version);

    esp_idf_svc::io::utils::copy(response, update, &mut buffer)?;

    Ok(())
}

fn read_firmware_info(
    buffer: &mut [u8],
    response: &mut Response<&mut EspHttpConnection>,
    update: &mut EspOtaUpdate,
) -> anyhow::Result<FirmwareInfo> {
    let update_info_load = EspFirmwareInfoLoad {};
    let mut update_info = FirmwareInfo {
        version: Default::default(),
        released: Default::default(),
        description: Default::default(),
        signature: Default::default(),
        download_id: Default::default(),
    };

    loop {
        let n = response.read(buffer)?;
        update.write(&buffer[0..n])?;
        if update_info_load.fetch(&buffer[0..n], &mut update_info)? {
            return Ok(update_info);
        }
    }
}
