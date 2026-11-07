//! The record a firmware image built on the platform carries of the board it was built for, the firmware and its
//! version, so that a flasher can tell, before it flashes a board, which board the firmware already on it was built
//! for, and refuse to flash firmware built for another: a wrong board's firmware drives the wrong pins, and once broke
//! a board's display.
//!
//! The record is put in ESP-IDF's `.rodata_custom_desc` section, which the linker places straight after the app
//! descriptor at the start of the app image, so that a flasher finds it in the first few hundred bytes of an app
//! partition, read over the serial port from the ROM bootloader, whether or not the firmware runs. It starts with
//! MAGIC, so that a flasher can tell it from whatever else is there, e.g. firmware not built on the platform. The
//! firmware also logs it as it starts.
//!
//! It is `no_std` and has no dependencies, so that both the firmware and the flasher, on a computer, use it.

#![no_std]

use core::fmt;

mod boards;
pub use boards::*;

/// What a record starts with, the platform's name and the record's format, which a change to the layout increases.
pub const MAGIC: [u8; 32] = *b"bruce-embedded-std firmware 1\0\0\0";

/// The length of each text field, which is padded with NULs.
pub const FIELD_LEN: usize = 64;

/// The size of a record, in bytes.
pub const SIZE: usize = MAGIC.len() + 3 * FIELD_LEN;

/// The record, as it is laid out in the image: MAGIC, then the board's, the firmware's and the version's names, each
/// UTF-8 padded with NULs to FIELD_LEN bytes.
#[repr(C)]
#[derive(Clone, PartialEq, Eq)]
pub struct FirmwareRecord {
    magic: [u8; 32],
    board: [u8; FIELD_LEN],
    firmware: [u8; FIELD_LEN],
    version: [u8; FIELD_LEN],
}

impl FirmwareRecord {
    /// The record of firmware built for the board, named e.g. `wave-esp32s3-rlcd-42`, the firmware, e.g. its crate's
    /// name, and its version. It is a const fn so that the record is a static the linker can place; a name longer
    /// than FIELD_LEN bytes fails the build.
    pub const fn new(board: &str, firmware: &str, version: &str) -> Self {
        FirmwareRecord { magic: MAGIC, board: field(board), firmware: field(firmware), version: field(version) }
    }

    /// The record at the start of the bytes, or None if they do not start with MAGIC, or a field is not UTF-8.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < SIZE || bytes[..MAGIC.len()] != MAGIC {
            return None;
        }
        let mut fields = bytes[MAGIC.len()..SIZE].chunks_exact(FIELD_LEN).map(|chunk| {
            let mut field = [0u8; FIELD_LEN];
            field.copy_from_slice(chunk);
            field
        });
        let record = FirmwareRecord {
            magic: MAGIC,
            board: fields.next()?,
            firmware: fields.next()?,
            version: fields.next()?,
        };
        (text(&record.board).is_some() && text(&record.firmware).is_some() && text(&record.version).is_some())
            .then_some(record)
    }

    /// The first record in the bytes, e.g. the start of an app partition or an image's section, wherever it is.
    pub fn find(bytes: &[u8]) -> Option<Self> {
        bytes.windows(MAGIC.len()).enumerate().filter(|(_, window)| *window == MAGIC).find_map(|(at, _)| Self::parse(&bytes[at..]))
    }

    /// The board the firmware was built for, e.g. `wave-esp32s3-rlcd-42`.
    pub fn board(&self) -> &str {
        text(&self.board).unwrap_or("")
    }

    /// The firmware, e.g. `marco-sparko-capture`.
    pub fn firmware(&self) -> &str {
        text(&self.firmware).unwrap_or("")
    }

    /// The firmware's version, e.g. `0.1.0`.
    pub fn version(&self) -> &str {
        text(&self.version).unwrap_or("")
    }
}

/// As the firmware logs it, starting with the magic, so that it can be found in a log.
impl fmt::Display for FirmwareRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: board={} firmware={} version={}", text(&self.magic).unwrap_or(""), self.board(), self.firmware(),
            self.version())
    }
}

impl fmt::Debug for FirmwareRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The text, padded with NULs, which fails the build, when called in a const, if it is too long.
const fn field(text: &str) -> [u8; FIELD_LEN] {
    let bytes = text.as_bytes();
    assert!(bytes.len() <= FIELD_LEN, "a firmware record's names are at most 64 bytes");
    let mut field = [0u8; FIELD_LEN];
    let mut i = 0;
    while i < bytes.len() {
        field[i] = bytes[i];
        i += 1;
    }
    field
}

/// The field's text, up to its first NUL, or None if it is not UTF-8.
fn text(field: &[u8]) -> Option<&str> {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    core::str::from_utf8(&field[..end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let record = FirmwareRecord::new("wave-esp32s3-rlcd-42", "marco-sparko-capture", "0.1.0");
        let bytes: [u8; SIZE] = unsafe { core::mem::transmute(record.clone()) };
        assert_eq!(FirmwareRecord::parse(&bytes), Some(record.clone()));
        assert_eq!(record.board(), "wave-esp32s3-rlcd-42");
        assert_eq!(record.firmware(), "marco-sparko-capture");
        assert_eq!(record.version(), "0.1.0");
    }

    #[test]
    fn found_after_other_bytes() {
        let record = FirmwareRecord::new("guition-jc3248w535c", "marco-sparko-capture", "0.1.0");
        let bytes: [u8; SIZE] = unsafe { core::mem::transmute(record.clone()) };
        let mut image = [0xffu8; 1024];
        image[0x120..0x120 + SIZE].copy_from_slice(&bytes);
        assert_eq!(FirmwareRecord::find(&image), Some(record));
    }

    #[test]
    fn absent() {
        assert_eq!(FirmwareRecord::find(&[0xffu8; 1024]), None);
        let mut bytes = [0u8; SIZE];
        bytes[..MAGIC.len()].copy_from_slice(&MAGIC);
        bytes[MAGIC.len()] = 0xff;
        assert_eq!(FirmwareRecord::parse(&bytes), None);
    }
}
