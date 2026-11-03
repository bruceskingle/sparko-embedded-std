//! The board's speaker's settings: its volume, which every sound plays at, see speaker.rs. The platform adds it on a
//! board with a speaker, see the speaker feature.

use std::time::Duration;

use esp_idf_svc::http::Method;
use esp_idf_svc::io::Read;
use log::warn;
use sparko_embedded_std::config::{ConfigSpec, ConfigSpecValue, FeatureConfig, TypedValue};

use crate::http::EspHttpServerManager;
use crate::speaker::Speaker;
use crate::{Esp32Platform, Esp32PlatformInitializer, Feature, FeatureDescriptor};

#[derive(FeatureConfig)]
#[config(label = "Speaker")]
pub struct SpeakerConfig {
    /// In percent, see Speaker::set_volume. 100 if not set. A slider, which plays a tone at each volume it is moved
    /// to, see PREVIEW.
    #[config(label = "Volume (%)", min = 0, max = 100, default = 100, preview = "/speaker/preview")]
    pub volume: Option<i32>,
}

pub struct SpeakerSettings;

impl Feature for SpeakerSettings {
    type Config = SpeakerConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor { name: "Speaker".to_string(), config: SpeakerConfig::to_config_spec()? })
    }

    fn start(&mut self, sparko: &mut Esp32Platform, _initializer: &mut Esp32PlatformInitializer, config: SpeakerConfig) -> anyhow::Result<()> {
        sparko.speaker().set_volume(volume(&config));
        Ok(())
    }
}

/// The volume config sets, in percent, 100 if it is not set.
pub(crate) fn volume(config: &SpeakerConfig) -> u8 {
    config.volume.unwrap_or(100).clamp(0, 100) as u8
}

/// The config page's volume slider POSTs each volume it is moved to here, as value=N, see Slider::preview.
const PREVIEW: &str = "/speaker/preview";

/// Plays a short tone at each volume POSTed to PREVIEW, without saving it, so that the slider's volume can be heard
/// before it is saved. Added with the config page, so that it works while the device is being set up too.
pub(crate) fn add_preview(server_manager: &mut EspHttpServerManager<'static>, speaker: Speaker) -> anyhow::Result<()> {
    server_manager.on(PREVIEW, Method::Post, move |mut request| {
        let mut body = [0u8; 32];
        let mut length = 0;
        while length < body.len() {
            match request.read(&mut body[length..])? {
                0 => break,
                read => length += read,
            }
        }
        let volume = url::form_urlencoded::parse(&body[..length])
            .find(|(name, _)| name == "value")
            .and_then(|(_, value)| value.parse::<i32>().ok());
        match volume {
            Some(volume) => {
                if let Err(error) = speaker.play_at(volume.clamp(0, 100) as u8, &crate::speaker::tone(880.0, Duration::from_millis(120))) {
                    warn!("Unable to play the volume's preview: {error:#}");
                }
                request.into_ok_response()?;
            }
            None => {
                request.into_status_response(400)?;
            }
        }
        Ok(())
    })?;
    Ok(())
}
