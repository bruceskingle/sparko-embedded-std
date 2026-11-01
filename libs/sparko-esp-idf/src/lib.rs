use std::fmt::Display;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use sparko_embedded_std::{
    config::{ConfigSpec, FeatureConfig},
    feature::FeatureDescriptor,
    graphics::Color,
};

mod esp32_platform;
pub use esp32_platform::*;

#[cfg(esp_idf_esp_https_server_enable)]
pub mod api;
#[cfg(feature = "sd-card")]
pub mod sd_card;

#[cfg(esp_idf_esp_https_server_enable)]
pub mod adoption;
#[cfg(esp_idf_esp_https_server_enable)]
mod identity;
#[cfg(esp_idf_esp_https_server_enable)]
mod memory;
mod starts;
pub mod panics;

/// The display's SPI device, on a bus which lives as long as the device, so that other devices can share it, e.g. the
/// SD card slot of the Waveshare ESP32-C6-Touch-LCD-1.47, see sd_card.rs.
#[cfg(feature = "mipi-dsi-display")]
pub type DisplaySpi = esp_idf_hal::spi::SpiDeviceDriver<'static, &'static esp_idf_hal::spi::SpiDriver<'static>>;

/// The firmware a board crate builds, which the platform cannot know itself, see Esp32PlatformBuilder::with_firmware.
#[derive(Clone, Copy)]
pub struct Firmware {
    pub name: &'static str,
    pub version: &'static str,
}

/// Why the device last started, e.g. Panic, or Brownout, which shows power dips, as /api/info says, see api.rs.
pub fn reset_reason() -> &'static str {
    #[allow(non_upper_case_globals)]
    match unsafe { esp_idf_sys::esp_reset_reason() } {
        esp_idf_sys::esp_reset_reason_t_ESP_RST_POWERON => "PowerOn",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_EXT => "ExternalPin",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_SW => "Software",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_PANIC => "Panic",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_INT_WDT => "InterruptWatchdog",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_TASK_WDT => "TaskWatchdog",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_WDT => "OtherWatchdog",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_DEEPSLEEP => "DeepSleep",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_BROWNOUT => "Brownout",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_SDIO => "Sdio",
        // Of newer chips, e.g. the ESP32-C6, which is reset over its own USB when it is flashed.
        esp_idf_sys::esp_reset_reason_t_ESP_RST_USB => "Usb",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_JTAG => "Jtag",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_EFUSE => "Efuse",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_PWR_GLITCH => "PowerGlitch",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_CPU_LOCKUP => "CpuLockup",
        _ => "Unknown",
    }
}

/// The board this was built for, from its board- feature.
pub fn board_name() -> &'static str {
    if cfg!(feature = "board-cyd") {
        "cyd"
    } else if cfg!(feature = "board-devkitv1") {
        "devkitv1"
    } else if cfg!(feature = "board-supermini-esp32c3") {
        "supermini-esp32c3"
    } else if cfg!(feature = "board-wave-esp32c6147") {
        "wave-esp32c6147"
    } else if cfg!(feature = "board-wave-esp32c6touch147") {
        "wave-esp32c6touch147"
    } else if cfg!(feature = "board-xiao-esp32c6") {
        "xiao-esp32c6"
    } else {
        "unknown"
    }
}

mod commands;
mod config_store;
mod core;
pub mod features;
mod http;
#[cfg(feature = "led")]
mod led;
mod mdns;
mod portal;
pub mod smart_led;
mod wifi;
// pub mod led_strip;
#[cfg(feature = "touch-driver")]
pub mod touch;

#[cfg(any(feature = "tilt", feature = "ahrs"))]
pub mod ahrs;

#[cfg(feature = "display")]
mod display;

pub trait AnyhowResultExt<T> {
    fn anyhow(self) -> anyhow::Result<T>;
}

impl<T, E> AnyhowResultExt<T> for Result<T, E>
where
    E: Display,
{
    fn anyhow(self) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::anyhow!("Operation failed: {}", e))
    }
}

// pub trait DynFeature {
//     fn do_init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;
//     fn start_with_config(
//         &mut self,
//         sparko: &mut Esp32Platform,
//         initializer: &mut Esp32PlatformInitializer,
//         config: &ConfigSpec,
//     ) -> anyhow::Result<()>;
// }

// pub trait Feature<C: FeatureConfig>: DynFeature {
//     fn start_with_config(
//         &mut self,
//         sparko: &mut Esp32Platform,
//         initializer: &mut Esp32PlatformInitializer,
//         config: &ConfigSpec,
//     ) -> anyhow::Result<()> {
//         let typed_config = C::from_config_spec(config)?;
//         self.start(sparko, initializer, typed_config)
//     }

//     fn init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;

//     fn start(
//         &mut self,
//         sparko: &mut Esp32Platform,
//         initializer: &mut Esp32PlatformInitializer,
//         config: C,
//     ) -> anyhow::Result<()>;
// }

pub trait DynFeature {
    fn do_init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;

    fn start_with_config(
        &mut self,
        sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &ConfigSpec,
    ) -> anyhow::Result<()>;
}

pub trait Feature {
    type Config: FeatureConfig;

    fn init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;

    fn start(
        &mut self,
        sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: Self::Config,
    ) -> anyhow::Result<()>;
}

impl<F: Feature> DynFeature for F {
    fn do_init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        F::init(self, init)
    }

    fn start_with_config(
        &mut self,
        sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &ConfigSpec,
    ) -> anyhow::Result<()> {
        let typed_config = F::Config::from_config_spec(config)?;
        self.start(sparko, initializer, typed_config)
    }
}

pub fn to_rgb565(color: &Color) -> Rgb565 {
    match color {
        Color::Black => Rgb565::BLACK,
        Color::Red => Rgb565::RED,
        Color::Green => Rgb565::GREEN,
        Color::Blue => Rgb565::BLUE,
        Color::Yellow => Rgb565::YELLOW,
        Color::Magenta => Rgb565::MAGENTA,
        Color::Cyan => Rgb565::CYAN,
        Color::White => Rgb565::WHITE,
    }
}

pub fn rgb565_from_rgb8(color: &rgb::RGB8) -> Rgb565 {
    Rgb565::new(color.r >> 3, color.g >> 2, color.b >> 3)
}
