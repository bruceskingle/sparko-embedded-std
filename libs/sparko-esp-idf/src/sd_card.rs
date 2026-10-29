//! The SD card slot of a board which has one, e.g. the CYD, on-board hardware which the platform owns when the
//! sd-card feature is enabled. The card is mounted with FAT at MOUNT_POINT at start-up, so that features can use it with
//! std::fs, see Esp32Platform::sd_card. A card which is missing or cannot be read is not an error for the device, only
//! for the features which want it.

use std::any::Any;

use esp_idf_hal::gpio::AnyIOPin;
use esp_idf_hal::sd::spi::SdSpiHostDriver;
use esp_idf_hal::sd::{SdCardConfiguration, SdCardDriver};
use esp_idf_hal::spi::{Dma, SpiDriver, SpiDriverConfig};
use esp_idf_svc::fs::fatfs::Fatfs;
use esp_idf_svc::io::vfs::MountedFatfs;
use log::info;

pub const MOUNT_POINT: &str = "/sd";
/// How many files may be open at once.
const MAX_OPEN_FILES: usize = 4;
/// The SPI clock, far slower than ESP-IDF's 20 MHz default, at which, and at 4 MHz, an old card in the CYD failed to
/// read and write, and plenty for files of records. 400 kHz is the speed every card must support while it starts.
/// TODO: find the fastest which works.
const SPEED_KHZ: u32 = 400;

/// Mounts the card, returning what must be kept for as long as it is mounted.
#[cfg(feature = "board-cyd")]
pub fn mount(
    spi: esp_idf_hal::spi::SPI3<'static>,
    sclk: esp_idf_hal::gpio::Gpio18<'static>,
    mosi: esp_idf_hal::gpio::Gpio23<'static>,
    miso: esp_idf_hal::gpio::Gpio19<'static>,
    cs: esp_idf_hal::gpio::Gpio5<'static>,
) -> anyhow::Result<Box<dyn Any>> {
    let spi_driver = SpiDriver::new(spi, sclk, mosi, Some(miso), &SpiDriverConfig::default().dma(Dma::Auto(4096)))?;
    // SD cards need pull-ups on their lines in SPI mode, which the CYD does not have, so the ESP32's own, as ESP-IDF's
    // examples do. Without them the card answers some commands and not others.
    for pin in [5, 18, 19, 23] {
        unsafe { esp_idf_sys::gpio_set_pull_mode(pin, esp_idf_sys::gpio_pull_mode_t_GPIO_PULLUP_ONLY) };
    }
    let mut config = SdCardConfiguration::new();
    config.speed_khz = SPEED_KHZ;
    let card = SdCardDriver::new_spi(
        SdSpiHostDriver::new(spi_driver, Some(cs), AnyIOPin::none(), AnyIOPin::none(), AnyIOPin::none(), None)?,
        &config,
    )?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    Ok(Box::new(mounted))
}
