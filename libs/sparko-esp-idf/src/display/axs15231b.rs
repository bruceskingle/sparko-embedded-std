//! The AXS15231B, the controller of the Guition JC3248W535C's 3.5" IPS LCD: 320x480, portrait, 16 bit colour, over
//! quad SPI (QSPI), which also has the capacitive touch, on I2C. It is drawn on in a framebuffer, in landscape, 480x320,
//! in PSRAM, which flush sends to the panel when it has changed, see spawn_flusher, rotated into the panel's rows, a
//! strip at a time through a small buffer in internal RAM which DMA can read.
//!
//! The panel is only sent whole frames, from its first row, as some of these panels ignore the commands which set the
//! area to draw in, so the first strip is a memory write, 0x2C, which starts at the top, and the rest memory write
//! continue, 0x3C, as Espressif's esp_lcd_axs15231b driver does. The start-up is sleep out and display on, which is
//! all this board needs, as ESPHome's users of it found, with 16 bit colour. Each command is sent as the panel's QSPI
//! framing has it: an opcode, 0x02 for a command and its parameters on one line, or 0x32 for pixels on four, and the
//! command in the middle of a 24 bit address, which ESP-IDF's esp_lcd SPI panel IO does in quad mode.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_idf_sys::{self as sys, esp};
use sparko_embedded_std::graphics::{Color, Console, DisplayManager};

/// The panel's own size, portrait.
const PANEL_WIDTH: usize = 320;
const PANEL_HEIGHT: usize = 480;
/// What is drawn on, landscape: the panel turned on its side, see ROTATED_CLOCKWISE.
pub const WIDTH: u32 = PANEL_HEIGHT as u32;
pub const HEIGHT: u32 = PANEL_WIDTH as u32;
/// Which way the landscape picture is turned onto the portrait panel, so that it is the right way up with the board's
/// USB port on the right, see send_strip.
const ROTATED_CLOCKWISE: bool = true;

/// The panel's rows sent at a time, through each of two buffers in internal RAM, 20 KB each.
const STRIP_ROWS: usize = 32;
const STRIP_BYTES: usize = STRIP_ROWS * PANEL_WIDTH * 2;
/// The QSPI clock, which Guition's own example uses.
const PIXEL_CLOCK_HZ: u32 = 40_000_000;
/// How often the flusher looks for changes to send, see spawn_flusher. A whole frame, 300 KB, takes about 16 ms at
/// 40 MHz on four lines.
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

/// The opcodes of the panel's QSPI framing, see the module comment.
const WRITE_COMMAND: u32 = 0x02;
const WRITE_PIXELS: u32 = 0x32;

/// The panel's pins: the QSPI bus's clock, chip select and four data lines, and the backlight's.
pub struct Pins {
    pub clock: i32,
    pub chip_select: i32,
    pub data: [i32; 4],
    pub backlight: i32,
}

/// The display, as a DrawTarget of Rgb565, landscape.
pub struct Axs15231b {
    io: sys::esp_lcd_panel_io_handle_t,
    /// A pixel each, landscape, row by row.
    frame: Vec<Rgb565>,
    /// The two strips, see STRIP_ROWS, used in turn, so that one is filled while the other is sent.
    strips: [*mut u8; 2],
    /// Whether the frame has changed since it was last sent, see flush.
    dirty: bool,
    /// The backlight's pin, see set_backlight.
    backlight: i32,
}

// The panel IO and strips are only used by whoever holds the display, behind the display manager's mutex.
unsafe impl Send for Axs15231b {}

impl Axs15231b {
    /// Starts the QSPI bus on SPI2 and the panel, and clears it to black, with the backlight off, which the platform
    /// turns on, unless the Screen feature's setting says to keep it off, see features/screen.rs.
    pub fn new(pins: Pins) -> anyhow::Result<Self> {
        let mut bus: sys::spi_bus_config_t = Default::default();
        bus.sclk_io_num = pins.clock;
        bus.__bindgen_anon_1.data0_io_num = pins.data[0];
        bus.__bindgen_anon_2.data1_io_num = pins.data[1];
        bus.__bindgen_anon_3.data2_io_num = pins.data[2];
        bus.__bindgen_anon_4.data3_io_num = pins.data[3];
        bus.data4_io_num = -1;
        bus.data5_io_num = -1;
        bus.data6_io_num = -1;
        bus.data7_io_num = -1;
        bus.max_transfer_sz = STRIP_BYTES as i32;
        esp!(unsafe { sys::spi_bus_initialize(sys::spi_host_device_t_SPI2_HOST, &bus, sys::spi_common_dma_t_SPI_DMA_CH_AUTO) })?;

        let mut config: sys::esp_lcd_panel_io_spi_config_t = Default::default();
        config.cs_gpio_num = pins.chip_select;
        config.dc_gpio_num = -1;
        config.spi_mode = 3;
        config.pclk_hz = PIXEL_CLOCK_HZ;
        // One in flight while the other strip is filled, see strips.
        config.trans_queue_depth = 1;
        config.lcd_cmd_bits = 32;
        config.lcd_param_bits = 8;
        config.flags.set_quad_mode(1);
        let mut io: sys::esp_lcd_panel_io_handle_t = std::ptr::null_mut();
        esp!(unsafe { sys::esp_lcd_new_panel_io_spi(sys::spi_host_device_t_SPI2_HOST as sys::esp_lcd_spi_bus_handle_t, &config, &mut io) })?;

        let strip = || -> anyhow::Result<*mut u8> {
            let strip = unsafe { sys::heap_caps_malloc(STRIP_BYTES, sys::MALLOC_CAP_DMA | sys::MALLOC_CAP_INTERNAL) } as *mut u8;
            anyhow::ensure!(!strip.is_null(), "no internal RAM for the display's strips");
            Ok(strip)
        };
        let mut display = Axs15231b {
            io,
            frame: vec![Rgb565::BLACK; WIDTH as usize * HEIGHT as usize],
            strips: [strip()?, strip()?],
            dirty: true,
            backlight: pins.backlight,
        };
        display.command(0x11, &[])?; // Sleep out
        std::thread::sleep(Duration::from_millis(120));
        display.command(0x3A, &[0x55])?; // 16 bit colour
        display.command(0x29, &[])?; // Display on
        std::thread::sleep(Duration::from_millis(20));
        display.flush()?;

        // The backlight driven off, as it may not be off if left floating, until set_backlight, once there is a
        // picture, so that what the panel had in its memory is not seen.
        let backlight = sys::gpio_config_t {
            pin_bit_mask: 1u64 << pins.backlight,
            mode: sys::gpio_mode_t_GPIO_MODE_OUTPUT,
            ..Default::default()
        };
        esp!(unsafe { sys::gpio_config(&backlight) })?;
        esp!(unsafe { sys::gpio_set_level(pins.backlight, 0) })?;
        Ok(display)
    }

    /// Turns the backlight on or off, e.g. off for a board run headless, whose panel is broken, see
    /// features/screen.rs.
    pub fn set_backlight(&mut self, on: bool) -> anyhow::Result<()> {
        esp!(unsafe { sys::gpio_set_level(self.backlight, on as u32) })?;
        Ok(())
    }

    fn command(&mut self, command: u8, parameters: &[u8]) -> anyhow::Result<()> {
        let lcd_command = ((WRITE_COMMAND << 24) | ((command as u32) << 8)) as i32;
        let (pointer, length) = if parameters.is_empty() { (std::ptr::null(), 0) } else { (parameters.as_ptr() as *const _, parameters.len()) };
        esp!(unsafe { sys::esp_lcd_panel_io_tx_param(self.io, lcd_command, pointer, length) })?;
        Ok(())
    }

    /// Fills strip with the panel's rows from first, each PANEL_WIDTH pixels, big endian, from the landscape frame.
    fn fill_strip(&self, strip: *mut u8, first: usize) {
        let strip = unsafe { std::slice::from_raw_parts_mut(strip, STRIP_BYTES) };
        let width = WIDTH as usize;
        for row in 0..STRIP_ROWS {
            let panel_y = first + row;
            for panel_x in 0..PANEL_WIDTH {
                // The panel's row is a column of the landscape frame.
                let (x, y) = if ROTATED_CLOCKWISE {
                    (panel_y, PANEL_WIDTH - 1 - panel_x)
                } else {
                    (PANEL_HEIGHT - 1 - panel_y, panel_x)
                };
                let pixel = self.frame[y * width + x].into_storage().to_be_bytes();
                let index = (row * PANEL_WIDTH + panel_x) * 2;
                strip[index] = pixel[0];
                strip[index + 1] = pixel[1];
            }
        }
    }

    /// Sends the frame to the panel, if it has changed, a strip at a time, see the module comment.
    pub fn flush(&mut self) -> anyhow::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        for (index, first) in (0..PANEL_HEIGHT).step_by(STRIP_ROWS).enumerate() {
            let strip = self.strips[index % 2];
            // The queue is one deep, so this strip's last send, two ago, has finished once the last one was queued.
            self.fill_strip(strip, first);
            let memory_write = if first == 0 { 0x2C } else { 0x3C };
            let lcd_command = ((WRITE_PIXELS << 24) | (memory_write << 8)) as i32;
            esp!(unsafe { sys::esp_lcd_panel_io_tx_color(self.io, lcd_command, strip as *const _, STRIP_BYTES) })?;
        }
        Ok(())
    }
}

impl OriginDimensions for Axs15231b {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

impl DrawTarget for Axs15231b {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 && (point.x as u32) < WIDTH && (point.y as u32) < HEIGHT {
                self.frame[point.y as usize * WIDTH as usize + point.x as usize] = color;
                self.dirty = true;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.frame.fill(color);
        self.dirty = true;
        Ok(())
    }
}

/// The colours, as the other colour displays have them, see to_rgb565 in sparko-embedded-std's graphics.
pub struct Axs15231bDisplayManager {
    pub display: Axs15231b,
    /// See DisplayManager::overlay_active and generation.
    pub overlay: bool,
    pub generation: u32,
}

impl DisplayManager for Axs15231bDisplayManager {
    type Display = Axs15231b;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    /// FONT_10X20, 48 characters across and 16 lines down, as the screen is large and sharp enough for it.
    fn console(&self) -> Console {
        Console { font: &embedded_graphics::mono_font::ascii::FONT_10X20, margin: 8 }
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

    fn map_color(&self, color: &Color) -> Rgb565 {
        sparko_embedded_std::graphics::to_rgb565(color)
    }

    fn map_rgb8(&self, color: &rgb::RGB8) -> Rgb565 {
        sparko_embedded_std::graphics::rgb565_from_rgb8(color)
    }
}

/// Sends the frame to the panel whenever it has changed, every FLUSH_INTERVAL, for as long as the device runs.
pub fn spawn_flusher(display_manager: &Arc<Mutex<Axs15231bDisplayManager>>) -> anyhow::Result<()> {
    let display_manager = display_manager.clone();
    std::thread::Builder::new().name("axs15231b flush".into()).stack_size(4096).spawn(move || loop {
        std::thread::sleep(FLUSH_INTERVAL);
        if let Err(error) = display_manager.lock().unwrap().display.flush() {
            log::warn!("Unable to update the display, {error:#}");
        }
    })?;
    Ok(())
}
