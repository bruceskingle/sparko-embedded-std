use std::fmt::Display;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use sparko_embedded_std::{
    config::{Config, ConfigSpec, FeatureConfig},
    feature::FeatureDescriptor,
    graphics::Color,
};

mod esp32_platform;
pub use esp32_platform::*;

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
