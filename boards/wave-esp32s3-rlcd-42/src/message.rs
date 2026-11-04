//! Shows a message on the whole screen for a second, e.g. when the KEY button is pressed, as an overlay, during which
//! the clock draws nothing, and after which the screen is cleared for it to draw again, see
//! DisplayManager::set_overlay.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use sparko_embedded_std::graphics::{Color, DisplayManager, wrap};
use sparko_esp_idf::BoardDisplayManager;

/// How long the message is shown.
const SHOWN_FOR: Duration = Duration::from_secs(1);

/// Shows text for SHOWN_FOR, holding the display only while drawing, so that it is sent to the panel meanwhile.
pub fn show(display_manager: &Arc<Mutex<BoardDisplayManager>>, text: &str) -> anyhow::Result<()> {
    {
        let mut display_manager = display_manager.lock().unwrap();
        display_manager.set_overlay(true)?;
        display_manager.fill_color(Color::Black)?;
        let display = display_manager.display();
        let area = display.bounding_box();
        let style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        // Wrapped to the width, and centred on the screen.
        let font = FONT_10X20.character_size;
        let lines = wrap(text, (area.size.width / font.width) as usize);
        let top = area.center().y - (lines.len() as i32 - 1) * font.height as i32 / 2;
        for (index, line) in lines.iter().enumerate() {
            let position = Point::new(area.center().x, top + index as i32 * font.height as i32);
            Text::with_alignment(line, position, style, Alignment::Center).draw(display)?;
        }
    }
    std::thread::sleep(SHOWN_FOR);
    display_manager.lock().unwrap().set_overlay(false)
}
