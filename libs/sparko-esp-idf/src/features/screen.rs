//! The screen's settings: whether to keep it off, e.g. on a board run headless whose panel is broken. A setting, rather
//! than a choice made when the firmware is built, so that the screen is on unless a device is told otherwise. The
//! platform adds it on a board whose backlight it drives, see the display-axs15231b and mipi-dsi-display features, e.g.
//! the Guition JC3248W535C and the CYD, and applies it as the board starts, see Esp32PlatformBuilder.

use sparko_embedded_std::config::{ConfigSpec, ConfigSpecValue, FeatureConfig, TypedValue};

use crate::{Esp32Platform, Esp32PlatformInitializer, Feature, FeatureDescriptor};

#[derive(FeatureConfig)]
#[config(label = "Screen")]
pub struct ScreenConfig {
    /// Whether to keep the backlight off, false if not set, so that the screen is on. Takes effect on a restart.
    #[config(label = "Keep the Screen Off")]
    pub off: Option<bool>,
}

pub struct ScreenSettings;

impl Feature for ScreenSettings {
    type Config = ScreenConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor { name: "Screen".to_string(), config: ScreenConfig::to_config_spec()? })
    }

    fn start(&mut self, _sparko: &mut Esp32Platform, _initializer: &mut Esp32PlatformInitializer, _config: ScreenConfig) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Whether config says to keep the screen on, true if it is not set.
pub(crate) fn on(config: &ScreenConfig) -> bool {
    !config.off.unwrap_or(false)
}
