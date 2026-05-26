#[cfg(feature = "mipi-dsi-display")]
pub mod mipi_dsi_display_manager;

#[cfg(feature = "display-jd9853")]
mod jd9853;

#[cfg(feature = "display-jd9853")]
pub use jd9853::*;
