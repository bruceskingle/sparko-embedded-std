pub mod binary_clock;
pub mod dyndns2;
#[cfg(any(feature = "display-axs15231b", feature = "mipi-dsi-display"))]
pub mod screen;
#[cfg(feature = "speaker")]
pub mod speaker_settings;
#[cfg(feature = "shtc3")]
pub mod temperature_humidity;

#[cfg(feature = "display")]
pub mod analog_clock;
