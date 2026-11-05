//! The boards' touch screens, each of which gives its listeners the points where it is touched, in the coordinates the
//! screen is drawn in.

/// A point where the screen is touched, in pixels from its top left, as it is drawn.
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
}

#[cfg(feature = "AXS5106L")]
pub mod axs5106l;
#[cfg(feature = "touch-axs15231b")]
pub mod axs15231b;
