use anyhow::Result;
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle, ascii::FONT_10X20};
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use rgb::RGB8;

use crate::{InitStatus, Status};

mod clock;
pub use clock::*;

pub enum Color {
    Black,
    Red,
    Green,
    Blue,
    Yellow,
    Magenta,
    Cyan,
    White,
}

pub trait DrawTargetResultExt<T> {
    fn anyhow(self) -> Result<T>;
}

impl<T, E> DrawTargetResultExt<T> for Result<T, E> {
    fn anyhow(self) -> Result<T> {
        self.map_err(|_| anyhow::anyhow!("DrawTarget operation failed"))
    }
}

/// How text is written on a board's screen, the same way by the platform, e.g. its status, see
/// DisplayManager::show_status, and by the features, e.g. lines of numbers, so that every device looks like the same
/// console, in a size which suits its screen, see DisplayManager::console.
#[derive(Clone, Copy)]
pub struct Console {
    pub font: &'static MonoFont<'static>,
    /// The space left around the edge of the screen.
    pub margin: u32,
}

impl Console {
    /// The height of a line, and the width of a character.
    pub fn line_height(&self) -> u32 {
        self.font.character_size.height
    }

    pub fn character_width(&self) -> u32 {
        self.font.character_size.width + self.font.character_spacing
    }

    /// How many characters fit across, and how many lines fit down, area, inside the margin.
    pub fn fit(&self, area: &embedded_graphics::primitives::Rectangle) -> (usize, usize) {
        let inside = |length: u32| length.saturating_sub(2 * self.margin);
        ((inside(area.size.width) / self.character_width()).max(1) as usize, (inside(area.size.height) / self.line_height()) as usize)
    }
}

impl Default for Console {
    /// FONT_10X20, which is 24 characters across a 240 pixel screen, and 32 across 320.
    fn default() -> Self {
        Console { font: &FONT_10X20, margin: 8 }
    }
}

pub trait DisplayManager {
    type Display: DrawTarget;

    fn display(&mut self) -> &mut Self::Display;

    /// Shows what the platform is doing, see show_status.
    fn set_status(&mut self, status: &Status) -> anyhow::Result<()> {
        self.show_status(status, &[])
    }

    /// How text is written on this board's screen, see Console.
    fn console(&self) -> Console {
        Console::default()
    }

    /// Shows what the platform is doing, e.g. joining the Wi-Fi, on the whole screen, in the console, as a title in a
    /// colour of its own, with details below it, e.g. the device's address, until it is running, when the screen is
    /// cleared for the features, see clear.
    fn show_status(&mut self, status: &Status, details: &[String]) -> anyhow::Result<()> {
        let (title, color) = match status {
            Status::Initializing(InitStatus::Starting) => ("Starting", Color::White),
            Status::Initializing(InitStatus::AwaitingClientIpAddress) => ("Joining the Wi-Fi", Color::Magenta),
            Status::Initializing(InitStatus::AwaitingTimeSync) => ("Getting the time", Color::Cyan),
            Status::Initializing(InitStatus::StartingFeatures) => ("Starting the features", Color::Green),
            Status::Running => return self.clear(),
            Status::Setup => ("Setup", Color::Yellow),
            Status::Error => ("Error", Color::Red),
        };
        self.fill_color(Color::Black)?;
        let console = self.console();
        let title_style = MonoTextStyle::new(console.font, self.map_color(&color));
        let detail_style = MonoTextStyle::new(console.font, self.map_color(&Color::White));
        let display = self.display();
        let area = display.bounding_box();
        let (across, down) = console.fit(&area);
        let top_left = area.top_left + Point::new(console.margin as i32, console.margin as i32);
        let lines = wrap(title, across).into_iter().map(|line| (line, title_style))
            .chain(details.iter().flat_map(|detail| wrap(detail, across)).map(|line| (line, detail_style)));
        for (index, (line, style)) in lines.take(down).enumerate() {
            let position = top_left + Point::new(0, (index as u32 * console.line_height()) as i32);
            Text::with_baseline(&line, position, style, Baseline::Top).draw(display).anyhow()?;
        }
        Ok(())
    }

    /// Clears the screen, after which renderers should draw everything again, see generation.
    fn clear(&mut self) -> anyhow::Result<()> {
        self.fill_color(Color::Black)
    }

    fn fill_color(&mut self, color: Color) -> anyhow::Result<()>;

    /// Whether the platform has the whole screen for now, e.g. to show adoption's pairing code, during which
    /// renderers, e.g. ClockRenderer, should draw nothing.
    fn overlay_active(&self) -> bool {
        false
    }

    /// Changes whenever the screen has been cleared, e.g. when an overlay ends, after which renderers should draw
    /// everything again, not only what has changed.
    fn generation(&self) -> u32 {
        0
    }

    /// Starts or ends an overlay, see overlay_active. Ending it clears the screen and changes the generation.
    fn set_overlay(&mut self, _active: bool) -> anyhow::Result<()> {
        Ok(())
    }

    fn map_color(&self, color: &Color) -> <Self::Display as DrawTarget>::Color;

    fn map_rgb8(&self, color: &RGB8) -> <Self::Display as DrawTarget>::Color;
}

/// text in lines of up to fit characters, e.g. for the console, broken between words where it can be.
pub fn wrap(text: &str, fit: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        if !line.is_empty() && line.chars().count() + 1 + word.len() > fit {
            lines.push(std::mem::take(&mut line));
        }
        // A word too long for a line of its own, e.g. a URL, is cut.
        while word.len() > fit {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            lines.push(word.drain(..fit).collect());
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.extend(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wraps_between_words() {
        assert_eq!(wrap("Join the Wi-Fi ESP32-Setup", 16), vec!["Join the Wi-Fi", "ESP32-Setup"]);
        assert_eq!(wrap("http://192.168.4.1/config", 10), vec!["http://192", ".168.4.1/c", "onfig"]);
        assert_eq!(wrap("", 10), Vec::<String>::new());
    }
}
