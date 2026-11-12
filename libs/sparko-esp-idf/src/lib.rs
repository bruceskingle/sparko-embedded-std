use std::fmt::Display;

use sparko_embedded_std::{
    config::{ConfigSpec, FeatureConfig},
    feature::FeatureDescriptor,
};

mod esp32_platform;
pub use esp32_platform::*;

#[cfg(esp_idf_esp_https_server_enable)]
pub mod api;
#[cfg(feature = "sd-card")]
pub mod sd_card;
#[cfg(feature = "shtc3")]
pub mod shtc3;
#[cfg(feature = "battery")]
pub mod battery;
pub mod push_button;
#[cfg(feature = "speaker")]
pub mod speaker;
pub mod chip;

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
#[cfg(any(feature = "mipi-dsi-display", feature = "display-st7305"))]
pub type DisplaySpi = esp_idf_hal::spi::SpiDeviceDriver<'static, &'static esp_idf_hal::spi::SpiDriver<'static>>;

pub use bruce_embedded_firmware_record::FirmwareRecord;

/// The firmware a board crate builds, which the platform cannot know itself, made with firmware!, see
/// Esp32PlatformBuilder::with_firmware.
#[derive(Clone, Copy)]
pub struct Firmware {
    pub name: &'static str,
    pub version: &'static str,
    /// The record in the image of the board it was built for, which a flasher reads before flashing, see
    /// bruce-embedded-firmware-record, or None if the board crate did not say what it builds.
    pub record: Option<&'static FirmwareRecord>,
}

/// The Firmware of the crate it is called in, its name and version, built for the board its `board-*` feature
/// chooses, with the record of them a flasher reads before flashing, placed by the linker at the start of the image,
/// e.g. `Esp32Platform::builder()...with_firmware(sparko_esp_idf::firmware!())`.
#[macro_export]
macro_rules! firmware {
    () => {{
        #[used]
        #[unsafe(link_section = ".rodata_custom_desc")]
        static RECORD: $crate::FirmwareRecord =
            $crate::FirmwareRecord::new($crate::board_name(), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        $crate::Firmware {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            // Through black_box, so that the record is read where it is, and so kept by the linker, which keeps only
            // what is referred to, rather than its contents copied by the compiler.
            record: Some(::core::hint::black_box(&RECORD)),
        }
    }};
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

#[cfg(all(feature = "boot-button", not(any(feature = "board-wave-esp32s3-rlcd-42", feature = "board-guition-jc3248w535c"))))]
compile_error!("The boot-button feature is for a board whose BOOT button the platform does not use: board-wave-esp32s3-rlcd-42, board-guition-jc3248w535c.");
#[cfg(all(feature = "key-button", not(feature = "board-wave-esp32s3-rlcd-42")))]
compile_error!("The key-button feature is for a board with a KEY button: board-wave-esp32s3-rlcd-42.");

/// The board this was built for, from its board- feature.
pub const fn board_name() -> &'static str {
    if cfg!(feature = "board-cyd-esp32-28") {
        "cyd-esp32-28"
    } else if cfg!(feature = "board-esp32-devkit-v1") {
        "esp32-devkit-v1"
    } else if cfg!(feature = "board-esp32c3-supermini") {
        "esp32c3-supermini"
    } else if cfg!(feature = "board-wave-esp32c6-lcd-147") {
        "wave-esp32c6-lcd-147"
    } else if cfg!(feature = "board-wave-esp32c6-touch-147") {
        "wave-esp32c6-touch-147"
    } else if cfg!(feature = "board-wave-esp32s3-rlcd-42") {
        "wave-esp32s3-rlcd-42"
    } else if cfg!(feature = "board-guition-jc3248w535c") {
        "guition-jc3248w535c"
    } else if cfg!(feature = "board-seeed-esp32c6-xiao") {
        "seeed-esp32c6-xiao"
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
mod portal;
pub mod smart_led;
mod wifi;
// pub mod led_strip;
#[cfg(feature = "touch-driver")]
pub mod touch;

/// The attitude and tilt, from sparko-embedded-std, and the IMU chips the boards have, which give it their readings.
#[cfg(any(feature = "tilt", feature = "ahrs"))]
pub mod ahrs {
    pub use sparko_embedded_std::ahrs::*;
    pub mod qmi8658;
}

#[cfg(feature = "display")]
mod display;
/// The board's display manager, the type of Esp32Platform::display_manager, for features outside the platform.
#[cfg(feature = "display")]
pub use display::BoardDisplayManager;

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
