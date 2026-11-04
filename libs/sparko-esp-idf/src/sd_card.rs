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
/// The SPI clock once the card has started, which is always at 400 kHz, half ESP-IDF's 20 MHz default. An old card in
/// the CYD failed to read and write even at 4 MHz, so 400 kHz was used, which made reading a day's records take
/// minutes. A good card is fine at this.
const SPEED_KHZ: u32 = 10_000;

/// Mounts the card, returning what must be kept for as long as it is mounted.
#[cfg(feature = "board-cyd-esp32-28")]
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

/// Mounts the card of a board whose slot shares the display's SPI bus, bus, with its chip select, cs, returning what
/// must be kept for as long as it is mounted. The display's device must be on the bus first, so that its chip select is
/// held high while the card starts.
#[cfg(feature = "board-wave-esp32c6-touch-147")]
pub fn mount_shared(
    bus: &'static SpiDriver<'static>,
    cs: esp_idf_hal::gpio::Gpio4<'static>,
) -> anyhow::Result<Box<dyn Any>> {
    // As on the CYD, see mount, the card's lines need pull-ups: MISO and its chip select, the others being the
    // display's too.
    for pin in [3, 4] {
        unsafe { esp_idf_sys::gpio_set_pull_mode(pin, esp_idf_sys::gpio_pull_mode_t_GPIO_PULLUP_ONLY) };
    }
    let mut config = SdCardConfiguration::new();
    config.speed_khz = SPEED_KHZ;
    let card = SdCardDriver::new_spi(
        SdSpiHostDriver::new(bus, Some(cs), AnyIOPin::none(), AnyIOPin::none(), AnyIOPin::none(), None)?,
        &config,
    )?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    Ok(Box::new(mounted))
}

/// Mounts the card of a board whose slot is wired for the SDMMC host, with one data line, as the Waveshare
/// ESP32-S3-RLCD-4.2's is, on GPIO21 (CMD), 38 (CLK) and 39 (D0), returning what must be kept for as long as it is
/// mounted.
#[cfg(feature = "board-wave-esp32s3-rlcd-42")]
pub fn mount_mmc(
    slot: esp_idf_hal::sd::mmc::SDMMC1<'static>,
    cmd: esp_idf_hal::gpio::Gpio21<'static>,
    clk: esp_idf_hal::gpio::Gpio38<'static>,
    d0: esp_idf_hal::gpio::Gpio39<'static>,
) -> anyhow::Result<Box<dyn Any>> {
    use esp_idf_hal::sd::mmc::{SdMmcHostConfiguration, SdMmcHostDriver};
    // The slot has no card detect or write protect lines. The ESP32's own pull-ups, as the board has none.
    let host = SdMmcHostDriver::new_1bit(slot, cmd, clk, d0, AnyIOPin::none(), AnyIOPin::none(), &SdMmcHostConfiguration::new())?;
    let card = SdCardDriver::new_mmc(host, &SdCardConfiguration::new())?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    Ok(Box::new(mounted))
}

/// The mounted card's size and free space, in bytes, from its FAT, None if there is no card mounted or it cannot be
/// read. FatFs counts the free space the first time, which may take a while on a large card, and keeps it up to date
/// after that.
pub fn space() -> Option<(u64, u64)> {
    let path = std::ffi::CString::new(MOUNT_POINT).ok()?;
    let (mut total, mut free) = (0u64, 0u64);
    let result = unsafe { esp_idf_sys::esp_vfs_fat_info(path.as_ptr(), &mut total, &mut free) };
    (result == esp_idf_sys::ESP_OK).then_some((total, free))
}
