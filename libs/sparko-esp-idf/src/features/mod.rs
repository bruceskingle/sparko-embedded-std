pub mod binary_clock;
pub mod dyndns2;
#[cfg(feature = "speaker")]
pub mod speaker_settings;
#[cfg(feature = "shtc3")]
pub mod temperature_humidity;

#[cfg(feature = "display")]
pub mod analog_clock;
