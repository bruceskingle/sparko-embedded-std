#[cfg(feature = "mipi-dsi-display")]
pub mod mipi_dsi_display_manager;
#[cfg(feature = "display-st7305")]
pub mod st7305;

/// The board's display manager, which features draw with, see Esp32Platform::display_manager.
#[cfg(feature = "mipi-dsi-display")]
pub type BoardDisplayManager = mipi_dsi_display_manager::MipiDsiDisplayManager;
#[cfg(feature = "display-st7305")]
pub type BoardDisplayManager = st7305::St7305DisplayManager;

#[cfg(feature = "display-jd9853")]
mod jd9853;

#[cfg(feature = "display-jd9853")]
pub use jd9853::*;
