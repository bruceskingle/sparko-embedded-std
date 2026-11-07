//! The boards the platform supports, by the id a firmware record names them with, so that a flasher can offer a list
//! of them by their full names, and ask the user whether a board really is the one they picked.

/// A board the platform supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardType {
    /// The id, as a firmware record names it and the `board-*` feature does without its prefix, e.g.
    /// `seeed-esp32c6-xiao`: the maker, the chip, then the maker's model, see the platform's CLAUDE.md.
    pub id: &'static str,
    /// The name its maker sells it by, e.g. "Seeed Studio XIAO ESP32C6", which the user is asked to check against.
    pub name: &'static str,
    /// The chip, as espflash names it, e.g. `esp32c6`, which the board's bootloader says, so that a flasher offers
    /// only the boards with the chip it finds, and never flashes firmware for another chip.
    pub chip: &'static str,
    /// The maker's page for it, or None if the maker has none, e.g. an unbranded board.
    pub url: Option<&'static str>,
    /// How to tell it from the others with the same chip, by looking at it.
    pub looks: &'static str,
    /// How to put it in its bootloader by hand, when a flasher cannot do it over USB.
    pub boot: &'static str,
}

/// The supported boards.
pub const BOARD_TYPES: &[BoardType] = &[
    BoardType {
        id: "cyd-esp32-28",
        name: "ESP32-2432S028R \"Cheap Yellow Display\"",
        chip: "esp32",
        url: None,
        looks: "A yellow board behind a 2.8 inch 320x240 resistive touch screen, with an SD card slot and an RGB LED.",
        boot: "Hold BOOT, press and release RST, then release BOOT.",
    },
    BoardType {
        id: "esp32-devkit-v1",
        name: "ESP32 DevKit V1",
        chip: "esp32",
        url: None,
        looks: "A bare ESP32-WROOM-32 module on a board with two rows of 15 pins, and EN and BOOT buttons.",
        boot: "Hold BOOT, press and release EN, then release BOOT.",
    },
    BoardType {
        id: "esp32c3-supermini",
        name: "ESP32-C3 SuperMini",
        chip: "esp32c3",
        url: None,
        looks: "A tiny board, 22.5 x 18 mm, with two rows of 8 pins, a ceramic antenna, and BOOT and RST buttons.",
        boot: "Hold BOOT, press and release RST, then release BOOT.",
    },
    BoardType {
        id: "wave-esp32c6-lcd-147",
        name: "Waveshare ESP32-C6-LCD-1.47",
        chip: "esp32c6",
        url: Some("https://www.waveshare.com/esp32-c6-lcd-1.47.htm"),
        looks: "A 1.47 inch 172x320 colour screen without touch, on a board with a TF card slot and an RGB LED.",
        boot: "Hold BOOT, press and release RESET, then release BOOT.",
    },
    BoardType {
        id: "wave-esp32c6-touch-147",
        name: "Waveshare ESP32-C6-Touch-LCD-1.47",
        chip: "esp32c6",
        url: Some("https://www.waveshare.com/esp32-c6-touch-lcd-1.47.htm"),
        looks: "A 1.47 inch 172x320 colour touch screen, on a board with a TF card slot and a motion sensor.",
        boot: "Hold BOOT, press and release RESET, then release BOOT.",
    },
    BoardType {
        id: "seeed-esp32c6-xiao",
        name: "Seeed Studio XIAO ESP32C6",
        chip: "esp32c6",
        url: Some("https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/"),
        looks: "A tiny board, 21 x 17.5 mm, with a metal shield, a USB-C port, and buttons marked B and R.",
        boot: "Hold B, press and release R, then release B.",
    },
    BoardType {
        id: "wave-esp32s3-rlcd-42",
        name: "Waveshare ESP32-S3-RLCD-4.2",
        chip: "esp32s3",
        url: Some("https://www.waveshare.com/esp32-s3-rlcd-4.2.htm"),
        looks: "A 4.2 inch black and white reflective screen, with an 18650 battery holder, and BOOT and KEY buttons.",
        boot: "Unplug it and take out its battery, then hold BOOT while plugging it in again.",
    },
    BoardType {
        id: "guition-jc3248w535c",
        name: "Guition JC3248W535C",
        chip: "esp32s3",
        url: None,
        looks: "A 3.5 inch 320x480 colour touch screen, with an SD card slot and a speaker connector on the back.",
        boot: "Hold BOOT, press and release RST, then release BOOT.",
    },
];

/// The supported board with the id, or None.
pub fn board_type(id: &str) -> Option<&'static BoardType> {
    BOARD_TYPES.iter().find(|board| board.id == id)
}

/// The supported boards with the chip, e.g. `esp32c6`, as a flasher offers them for a board it found.
pub fn board_types_with_chip(chip: &str) -> impl Iterator<Item = &'static BoardType> {
    BOARD_TYPES.iter().filter(move |board| board.chip == chip)
}
