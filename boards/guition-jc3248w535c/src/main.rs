//! The platform on the Guition JC3248W535C: an analog clock on its 3.5" colour display, and a test pattern, see
//! test_pattern.rs, to enable on the /config page in place of the clock, which shows which way up the display is and
//! that its colours are right, and a dot where the screen is touched, see TouchDots, to check the touch's coordinates.
//! BOOT shows the SD card's size and free space, or why there is no card.

mod test_pattern;

use std::sync::{Arc, Mutex};

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
    text::Text,
};
use sparko_embedded_std::listener::Listener;
use sparko_esp_idf::{BoardDisplayManager, Esp32Platform, features::analog_clock::AnalogClock, touch::TouchPoint};

/// Draws a yellow dot where the screen is touched, so that a finger drawn across it leaves a trail under it if the
/// touch's coordinates are right.
struct TouchDots {
    display_manager: Arc<Mutex<BoardDisplayManager>>,
}

impl Listener<TouchPoint> for TouchDots {
    fn on_event(&self, point: &TouchPoint) {
        let mut display_manager = self.display_manager.lock().unwrap();
        let _ = Circle::with_center(Point::new(point.x as i32, point.y as i32), 9)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::YELLOW))
            .draw(&mut display_manager.display);
    }
}

fn main() -> anyhow::Result<()> {
    let (builder, _remainder) = Esp32Platform::builder()?;

    let platform = builder
        .with_feature(Box::new(AnalogClock::builder().build()?))?
        .with_feature(Box::new(test_pattern::TestPattern))?
        .with_boot_button(|sparko| {
            let display_manager = sparko.display_manager.clone();
            let sd_card = sparko.sd_card();
            Ok(Box::new(move || {
                let text = match (&sd_card, sparko_esp_idf::sd_card::space()) {
                    (Ok(_), Some((total, free))) => format!("SD {:.1} GB, {:.1} GB free", total as f64 / 1e9, free as f64 / 1e9),
                    (Ok(_), None) => "SD card cannot be read".to_string(),
                    (Err(error), _) => format!("No SD card: {error}"),
                };
                let mut display_manager = display_manager.lock().unwrap();
                let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
                let _ = Text::new(&text, Point::new(10, 300), style).draw(&mut display_manager.display);
                Ok(())
            }))
        })
        .with_firmware(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
        .build()?;

    // The touch driver keeps only a weak reference to its listeners, so this one is kept for as long as main runs.
    let dots: Arc<dyn Listener<TouchPoint>> = Arc::new(TouchDots { display_manager: platform.sparko_std.display_manager.clone() });
    platform.sparko_std.touch_driver.add_listener(&dots);
    platform.start()
}
