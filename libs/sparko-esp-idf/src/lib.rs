use std::fmt::Display;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use sparko_embedded_std::{config::Config, feature::FeatureDescriptor, graphics::Color};

pub mod sparko_esp32_std;

mod commands;
mod config_store;
mod core;
pub mod dyndns2;
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

pub mod binary_clock_feature;

#[cfg(feature = "display")]
pub mod analog_clock_feature;

#[cfg(feature = "mipi-dsi-display")]
mod display_mipidsi;

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

pub trait Feature {
    fn init(
        &self,
        init: &mut sparko_esp32_std::SparkoEsp32StdInitializer,
    ) -> anyhow::Result<FeatureDescriptor>;
    fn start(
        &mut self,
        sparko: &mut sparko_esp32_std::SparkoEsp32Std,
        initializer: &mut sparko_esp32_std::SparkoEsp32StdInitializer,
        config: &Config,
    ) -> anyhow::Result<()>;
}

pub trait FeatureConfig {}

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
