//! The Sitronix ST7305, the controller of the Waveshare ESP32-S3-RLCD-4.2's 4.2" reflective LCD: 400x300, black and
//! white, over SPI, with no backlight. Unlike the colour displays, see mipi_dsi_display_manager.rs, which are drawn on
//! directly, it is drawn on in a framebuffer, of a bit a pixel, which flush sends to the panel when it has changed, see
//! spawn_flusher. The panel's start-up commands and the framebuffer's layout are Waveshare's, from its examples'
//! display_bsp.cpp.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use esp_idf_hal::gpio::{Output, PinDriver};
use sparko_embedded_std::graphics::{Color, DisplayManager};

pub const WIDTH: u32 = 400;
pub const HEIGHT: u32 = 300;
/// How many pixels in from each edge are hidden by the panel's frame, on the Waveshare ESP32-S3-RLCD-4.2 the outermost
/// row and column of each (found with a test pattern, 3 Nov 2026). The DrawTarget is only the visible area within
/// them, so that nothing is drawn where it cannot be seen.
pub const HIDDEN: u32 = 1;
/// A bit a pixel.
const BUFFER_LEN: usize = (WIDTH * HEIGHT / 8) as usize;
/// How often the flusher looks for changes to send, see spawn_flusher. Sending the whole framebuffer takes about 12 ms
/// at 10 MHz.
const FLUSH_INTERVAL: Duration = Duration::from_millis(200);

/// The panel's start-up commands, each with its parameters, and how long to wait after it, in ms.
const INIT: &[(u8, &[u8], u32)] = &[
    (0xD6, &[0x17, 0x02], 0), // NVM load control
    (0xD1, &[0x01], 0),       // Booster enable
    (0xC0, &[0x11, 0x04], 0), // Gate voltage
    (0xC1, &[0x69, 0x69, 0x69, 0x69], 0),
    (0xC2, &[0x19, 0x19, 0x19, 0x19], 0),
    (0xC4, &[0x4B, 0x4B, 0x4B, 0x4B], 0),
    (0xC5, &[0x19, 0x19, 0x19, 0x19], 0),
    (0xD8, &[0x80, 0xE9], 0),
    (0xB2, &[0x02], 0),
    (0xB3, &[0xE5, 0xF6, 0x05, 0x46, 0x77, 0x77, 0x77, 0x77, 0x76, 0x45], 0),
    (0xB4, &[0x05, 0x46, 0x77, 0x77, 0x77, 0x77, 0x76, 0x45], 0),
    (0x62, &[0x32, 0x03, 0x1F], 0),
    (0xB7, &[0x13], 0),
    (0xB0, &[0x64], 0),
    (0x11, &[], 200), // Sleep out
    (0xC9, &[0x00], 0),
    (0x36, &[0x48], 0), // Memory access control
    (0x3A, &[0x11], 0), // Pixel format
    (0xB9, &[0x20], 0),
    (0xB8, &[0x29], 0),
    (0x21, &[], 0), // Inversion on
    (0x2A, &[0x12, 0x2A], 0),
    (0x2B, &[0x00, 0xC7], 0),
    (0x35, &[0x00], 0), // Tearing effect line on
    (0xD0, &[0xFF], 0),
    (0x38, &[], 0), // Idle mode off
    (0x29, &[], 0), // Display on
];

/// The display, as a DrawTarget of BinaryColor, in landscape: On is ink, black, and Off is paper, white. It is the
/// visible area, WIDTH and HEIGHT less HIDDEN on each side, whose top left is the panel's (HIDDEN, HIDDEN).
pub struct St7305 {
    spi: crate::DisplaySpi,
    dc: PinDriver<'static, Output>,
    /// A bit a pixel, set for white, laid out as the panel wants it, see index.
    buffer: Vec<u8>,
    /// Whether the buffer has changed since it was last sent, see flush.
    dirty: bool,
}

impl St7305 {
    /// Resets the panel with reset, starts it, and clears it to white.
    pub fn new(spi: crate::DisplaySpi, dc: PinDriver<'static, Output>, mut reset: PinDriver<'static, Output>) -> anyhow::Result<Self> {
        reset.set_high()?;
        std::thread::sleep(Duration::from_millis(50));
        reset.set_low()?;
        std::thread::sleep(Duration::from_millis(20));
        reset.set_high()?;
        std::thread::sleep(Duration::from_millis(50));
        // The panel is not reset again, so its pin is left high.
        std::mem::forget(reset);
        let mut display = St7305 { spi, dc, buffer: vec![0xFF; BUFFER_LEN], dirty: true };
        for (command, parameters, delay) in INIT {
            display.command(*command, parameters)?;
            if *delay > 0 {
                std::thread::sleep(Duration::from_millis(*delay as u64));
            }
        }
        display.flush()?;
        Ok(display)
    }

    fn command(&mut self, command: u8, parameters: &[u8]) -> anyhow::Result<()> {
        self.dc.set_low()?;
        self.spi.write(&[command])?;
        if !parameters.is_empty() {
            self.dc.set_high()?;
            self.spi.write(parameters)?;
        }
        Ok(())
    }

    /// Sends the framebuffer to the panel, if it has changed.
    pub fn flush(&mut self) -> anyhow::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.command(0x2A, &[0x12, 0x2A])?; // Columns
        self.command(0x2B, &[0x00, 0xC7])?; // Rows
        self.command(0x2C, &[])?; // Memory write
        self.dc.set_high()?;
        self.spi.write(&self.buffer)?;
        self.dirty = false;
        Ok(())
    }

    /// The pixel's byte in the buffer, and its bit: the panel takes the landscape screen in columns two pixels wide,
    /// from the bottom up, four rows to a byte.
    fn index(x: u32, y: u32) -> (usize, u8) {
        let inverted_y = HEIGHT - 1 - y;
        let index = (x / 2) * (HEIGHT / 4) + inverted_y / 4;
        let bit = 7 - ((inverted_y % 4) * 2 + x % 2);
        (index as usize, 1 << bit)
    }

    fn set(&mut self, x: u32, y: u32, color: BinaryColor) {
        let (index, mask) = Self::index(x, y);
        match color {
            BinaryColor::On => self.buffer[index] &= !mask,
            BinaryColor::Off => self.buffer[index] |= mask,
        }
    }
}

impl OriginDimensions for St7305 {
    fn size(&self) -> Size {
        Size::new(WIDTH - 2 * HIDDEN, HEIGHT - 2 * HIDDEN)
    }
}

impl DrawTarget for St7305 {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let size = self.size();
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 && (point.x as u32) < size.width && (point.y as u32) < size.height {
                self.set(point.x as u32 + HIDDEN, point.y as u32 + HIDDEN, color);
                self.dirty = true;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.buffer.fill(if color == BinaryColor::On { 0x00 } else { 0xFF });
        self.dirty = true;
        Ok(())
    }
}

/// Draws in ink on paper: the colour displays' dark backgrounds are paper, white, and anything brighter is ink, black,
/// which reads best on a reflective screen.
pub struct St7305DisplayManager {
    pub display: St7305,
    /// See DisplayManager::overlay_active and generation.
    pub overlay: bool,
    pub generation: u32,
}

impl DisplayManager for St7305DisplayManager {
    type Display = St7305;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn overlay_active(&self) -> bool {
        self.overlay
    }

    fn generation(&self) -> u32 {
        self.generation
    }

    fn set_overlay(&mut self, active: bool) -> anyhow::Result<()> {
        if self.overlay && !active {
            self.clear()?;
        }
        self.overlay = active;
        Ok(())
    }

    fn clear(&mut self) -> anyhow::Result<()> {
        self.fill_color(Color::Black)?;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    fn fill_color(&mut self, color: Color) -> anyhow::Result<()> {
        let color = self.map_color(&color);
        let _ = self.display.clear(color);
        Ok(())
    }

    fn map_color(&self, color: &Color) -> BinaryColor {
        match color {
            Color::Black => BinaryColor::Off,
            _ => BinaryColor::On,
        }
    }

    fn map_rgb8(&self, color: &rgb::RGB8) -> BinaryColor {
        let luminance = (color.r as u32 * 299 + color.g as u32 * 587 + color.b as u32 * 114) / 1000;
        if luminance < 64 { BinaryColor::Off } else { BinaryColor::On }
    }
}

/// Sends the framebuffer to the panel whenever it has changed, every FLUSH_INTERVAL, for as long as the device runs.
pub fn spawn_flusher(display_manager: &Arc<Mutex<St7305DisplayManager>>) -> anyhow::Result<()> {
    let display_manager = display_manager.clone();
    std::thread::Builder::new().name("st7305 flush".into()).stack_size(4096).spawn(move || loop {
        std::thread::sleep(FLUSH_INTERVAL);
        if let Err(error) = display_manager.lock().unwrap().display.flush() {
            log::warn!("Unable to update the display, {error:#}");
        }
    })?;
    Ok(())
}
