use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use esp_idf_hal::{
    gpio::{Output, PinDriver},
    spi::SpiDeviceDriver,
};
use sparko_embedded_std::graphics::{Color, DisplayManager};

#[cfg(feature = "board-cyd")]
use crate::display::mipi_dsi_display_manager;
use crate::{rgb565_from_rgb8, to_rgb565};

pub struct EspDi {
    pub spi: SpiDeviceDriver<'static, esp_idf_hal::spi::SpiDriver<'static>>,
    pub dc: PinDriver<'static, Output>,
    pub xoffset: i16,
    pub yoffset: i16,
}

impl mipidsi::interface::Interface for EspDi {
    type Word = u8;
    type Error = esp_idf_hal::spi::SpiError;
    const KIND: mipidsi::interface::InterfaceKind = mipidsi::interface::InterfaceKind::Serial4Line;

    fn send_command(&mut self, cmd: u8, args: &[u8]) -> Result<(), Self::Error> {
        self.dc.set_low().ok();
        self.spi.write(&[cmd])?;

        if !args.is_empty() {
            self.dc.set_high().ok();

            match cmd {
                0x2A => {
                    if self.xoffset == 0 {
                        self.spi.write(args)?;
                    } else {
                        // CASET: apply X offset
                        let mut buf = [0u8; 4];
                        buf.copy_from_slice(args);

                        let (start, end) = if self.xoffset >= 0 {
                            (
                                u16::from_be_bytes([buf[0], buf[1]])
                                    .saturating_add(self.xoffset as u16),
                                u16::from_be_bytes([buf[2], buf[3]])
                                    .saturating_add(self.xoffset as u16),
                            )
                        } else {
                            (
                                u16::from_be_bytes([buf[0], buf[1]])
                                    .saturating_sub(-self.xoffset as u16),
                                u16::from_be_bytes([buf[2], buf[3]])
                                    .saturating_sub(-self.xoffset as u16),
                            )
                        };

                        let adj = [(start >> 8) as u8, start as u8, (end >> 8) as u8, end as u8];

                        self.spi.write(&adj)?;
                    }
                }

                0x2B => {
                    if self.yoffset == 0 {
                        self.spi.write(args)?;
                    } else {
                        // RASET: apply X offset
                        let mut buf = [0u8; 4];
                        buf.copy_from_slice(args);

                        let (start, end) = if self.yoffset >= 0 {
                            (
                                u16::from_be_bytes([buf[0], buf[1]])
                                    .saturating_add(self.yoffset as u16),
                                u16::from_be_bytes([buf[2], buf[3]])
                                    .saturating_add(self.yoffset as u16),
                            )
                        } else {
                            (
                                u16::from_be_bytes([buf[0], buf[1]])
                                    .saturating_sub(-self.yoffset as u16),
                                u16::from_be_bytes([buf[2], buf[3]])
                                    .saturating_sub(-self.yoffset as u16),
                            )
                        };

                        let adj = [(start >> 8) as u8, start as u8, (end >> 8) as u8, end as u8];

                        self.spi.write(&adj)?;
                    }
                }

                _ => {
                    self.spi.write(args)?;
                }
            }
        }

        Ok(())
    }

    fn send_pixels<const N: usize>(
        &mut self,
        pixels: impl IntoIterator<Item = [u8; N]>,
    ) -> Result<(), Self::Error> {
        self.dc.set_high().ok();

        for chunk in pixels {
            self.spi.write(&chunk)?;
        }

        Ok(())
    }

    fn send_repeated_pixel<const N: usize>(
        &mut self,
        pixel: [u8; N],
        mut count: u32,
    ) -> Result<(), Self::Error> {
        self.dc.set_high().ok();

        let mut buf = [0u8; 64];

        while count > 0 {
            let n = core::cmp::min(count, (buf.len() / N) as u32);
            let mut idx = 0;

            for _ in 0..n {
                for b in pixel {
                    buf[idx] = b;
                    idx += 1;
                }
            }

            self.spi.write(&buf[..idx])?;
            count -= n;
        }

        Ok(())
    }
}

pub struct MipiDsiDisplayManager {
    pub back_light: PinDriver<'static, Output>,
    #[cfg(feature = "board-cyd")]
    pub display: mipidsi::Display<EspDi, mipidsi::models::ILI9341Rgb565, mipidsi::NoResetPin>,
    #[cfg(feature = "board-wave-esp32c6touch147")]
    // pub display: mipidsi::Display<crate::display_mipidsi::EspDi, mipidsi::models::ILI9341Rgb565, PinDriver<'static, esp_idf_hal::gpio::Output>>,
    pub display: mipidsi::Display<
        EspDi,
        mipidsi::models::ST7789,
        PinDriver<'static, esp_idf_hal::gpio::Output>,
    >,
    #[cfg(feature = "board-wave-esp32c6147")]
    pub display: mipidsi::Display<
        EspDi,
        mipidsi::models::ST7789,
        PinDriver<'static, esp_idf_hal::gpio::Output>,
    >,
}

impl MipiDsiDisplayManager {}

impl DisplayManager for MipiDsiDisplayManager {
    #[cfg(feature = "board-cyd")]
    type Display = mipidsi::Display<EspDi, mipidsi::models::ILI9341Rgb565, mipidsi::NoResetPin>;
    #[cfg(feature = "board-wave-esp32c6touch147")]
    // type Display = mipidsi::Display<crate::display_mipidsi::EspDi, mipidsi::models::ILI9341Rgb565, PinDriver<'static, esp_idf_hal::gpio::Output>>;
    type Display = mipidsi::Display<
        EspDi,
        mipidsi::models::ST7789,
        PinDriver<'static, esp_idf_hal::gpio::Output>,
    >;
    #[cfg(feature = "board-wave-esp32c6147")]
    type Display = mipidsi::Display<
        EspDi,
        mipidsi::models::ST7789,
        PinDriver<'static, esp_idf_hal::gpio::Output>,
    >;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn fill_color(&mut self, color: Color) -> anyhow::Result<()> {
        self.display
            .bounding_box()
            // Rectangle::new(Point::new(0, 0), self.size)
            .into_styled(PrimitiveStyle::with_fill(to_rgb565(&color)))
            .draw(&mut self.display)?;
        Ok(())
    }

    fn map_color(&self, color: &Color) -> <Self::Display as DrawTarget>::Color {
        to_rgb565(color)
    }

    fn map_rgb8(&self, color: &rgb::RGB8) -> <Self::Display as DrawTarget>::Color {
        rgb565_from_rgb8(color)
    }
}
