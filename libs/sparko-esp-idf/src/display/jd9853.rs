use embedded_hal::delay::DelayNs;
use mipidsi::interface::Interface;



pub fn jd9853_init<DI, DELAY>(
    di: &mut DI,
    delay: &mut DELAY,
) -> Result<(), DI::Error>
where
    DI: Interface,
    DELAY: DelayNs,
{
    let mut cmd = |c: u8, data: &[u8]| di.send_command(c, data);

    // Sleep out
    cmd(0x11, &[])?;
    delay.delay_ns(120_000_000);

    // Identification unlock
    cmd(0xDF, &[0x98, 0x53])?;
    cmd(0xDF, &[0x98, 0x53])?;

    cmd(0xB2, &[0x23])?;
    cmd(0xB7, &[0x00, 0x47, 0x00, 0x6F])?;

    cmd(0xBB, &[0x1C, 0x1A, 0x55, 0x73, 0x63, 0xF0])?;
    cmd(0xC0, &[0x44, 0xA4])?;
    cmd(0xC1, &[0x16])?;

    cmd(0xC3, &[0x7D, 0x07, 0x14, 0x06, 0xCF, 0x71, 0x72, 0x77])?;

    cmd(
        0xC4,
        &[0x00, 0x00, 0xA0, 0x79, 0x0B, 0x0A, 0x16, 0x79, 0x0B, 0x0A, 0x16, 0x82],
    )?;

    // Gamma
    cmd(
        0xC8,
        &[
            0x3F, 0x32, 0x29, 0x29, 0x27, 0x2B, 0x27, 0x28,
            0x28, 0x26, 0x25, 0x17, 0x12, 0x0D, 0x04, 0x00,
            0x3F, 0x32, 0x29, 0x29, 0x27, 0x2B, 0x27, 0x28,
            0x28, 0x26, 0x25, 0x17, 0x12, 0x0D, 0x04, 0x00,
        ],
    )?;

    cmd(0xD0, &[0x04, 0x06, 0x6B, 0x0F, 0x00])?;
    cmd(0xD7, &[0x00, 0x30])?;
    cmd(0xE6, &[0x14])?;
    cmd(0xDE, &[0x01])?;

    cmd(0xB7, &[0x03, 0x13, 0xEF, 0x35, 0x35])?;
    cmd(0xC1, &[0x14, 0x15, 0xC0])?;
    cmd(0xC2, &[0x06, 0x3A])?;
    cmd(0xC4, &[0x72, 0x12])?;

    cmd(0xBE, &[0x00])?;
    cmd(0xDE, &[0x02])?;
    cmd(0xE5, &[0x00, 0x02, 0x00])?;
    cmd(0xE5, &[0x01, 0x02, 0x00])?;
    cmd(0xDE, &[0x00])?;

    cmd(0x35, &[0x00])?;

    // RGB565
    cmd(0x3A, &[0x05])?;

    // Column/row range (includes 34px X offset)
    // cmd(0x2A, &[0x00, 0x22, 0x00, 0xCD])?;       // 👉 Remove that from init Because mipidsi will: dynamically set CASET/RASET for every draw 
                                                    // If you leave it: you’ll get clipping / weird rendering
    cmd(0x2B, &[0x00, 0x00, 0x01, 0x3F])?;

    cmd(0x29, &[])?; // Display ON

    Ok(())
}

use embedded_graphics::pixelcolor::Rgb565;
use mipidsi::{Builder};
use mipidsi::options::Orientation;

// pub fn create_display<'a, DI, DELAY>(
//     mut di: DI,
//     delay: &mut DELAY,
// ) -> Result<mipidsi::Display<DI, Rgb565>, DI::Error>
// where
//     DI: mipidsi::interface::Interface,
//     DELAY: embedded_hal::delay::DelayNs,
// {
//     // 1. Run vendor init FIRST
//     jd9853_init(&mut di, delay)?;

//     // 2. Build mipidsi display WITHOUT model
//     let mut display = Builder::new(di, Rgb565)
//         .init(delay)
//         .unwrap();

//     // 3. Critical: fix X offset (JD9853 quirk)
//     display.set_offset(34, 0);

//     // 4. Optional: orientation tuning
//     display.set_orientation(Orientation::new());

//     Ok(display)
// }