//! The boards identified on this computer, kept in a file shared by every app which flashes boards, so that the list
//! of ports can say what is probably on each without touching it: for sure, by ID, for a board on its chip's own USB,
//! whose serial number is its ID, and only maybe, by its port, for one on a USB serial chip, as another board may have
//! been plugged into the same socket since. macOS names a port for its socket, or for its USB serial chip's serial
//! number, which many chips share, so a port's name alone is only a hint.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serialport::UsbPortInfo;

use crate::{Board, Port, a, board_type};

/// A board, as it was when last identified, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    /// Its ID, its Wi-Fi MAC in hex.
    pub id: String,
    pub chip: String,
    /// The board, firmware and version its firmware record said, if it had one.
    pub board: Option<String>,
    pub firmware: Option<String>,
    pub version: Option<String>,
    /// The port it was on, and its USB vendor, product and serial number.
    pub port: String,
    pub vid: u16,
    pub pid: u16,
    pub serial_number: Option<String>,
    /// When, as unix seconds.
    pub at: u64,
}

/// What a port's board probably is, from the boards seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guess {
    /// The board with the port's ID, as it was when last identified.
    Known(Seen),
    /// The board last identified on the port, which may since have been unplugged and another plugged in.
    Maybe(Seen),
}

impl Guess {
    /// What the board probably is, for the user, e.g. "Maybe a Seeed Studio XIAO ESP32C6, ID 98a316bb08c4".
    pub fn describe(&self) -> String {
        match self {
            Guess::Known(seen) => format!("{}, when last identified", describe(seen)),
            Guess::Maybe(seen) => format!("Maybe {}, ID {}", lower_first(&describe(seen)), seen.id),
        }
    }
}

/// What the seen board was, e.g. "A Seeed Studio XIAO ESP32C6, running demo 0.1.0".
fn describe(seen: &Seen) -> String {
    match (&seen.board, &seen.firmware, &seen.version) {
        (Some(board), Some(firmware), Some(version)) => {
            let name = board_type(board).map_or(board.as_str(), |board_type| board_type.name);
            let name = a(name);
            format!("{}{}, running {firmware} {version}", name[..1].to_uppercase(), &name[1..])
        }
        _ => format!("An {} whose firmware did not say which board it is", seen.chip),
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_lowercase().chain(chars).collect()).unwrap_or_default()
}

/// The file the boards seen are kept in, in the user's data directory, beside the shared firmware folder.
fn file() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("bruce-embedded-std").join("boards-seen.json"))
}

/// The boards seen, or none if the file cannot be read.
pub fn load() -> Vec<Seen> {
    file().and_then(|file| std::fs::read(file).ok()).and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

/// Remembers the board, identified on the port with the USB details, in place of what was seen of it before, and of
/// whatever was last seen on the same port.
pub fn remember(board: &Board, usb: &UsbPortInfo) -> anyhow::Result<()> {
    let Some(file) = file() else { return Ok(()) };
    let record = board.apps.iter().find_map(|app| app.record.as_ref());
    let seen = Seen {
        id: board.id.clone(),
        chip: board.chip.clone(),
        board: record.map(|record| record.board().to_string()),
        firmware: record.map(|record| record.firmware().to_string()),
        version: record.map(|record| record.version().to_string()),
        port: board.port.clone(),
        vid: usb.vid,
        pid: usb.pid,
        serial_number: usb.serial_number.clone(),
        at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs()),
    };
    let mut all = load();
    all.retain(|other| other.id != seen.id && !same_port(other, &seen.port, usb));
    all.push(seen);
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    // Written beside it and moved over it, so that it is never left half written.
    let written = file.with_extension("json.new");
    std::fs::write(&written, serde_json::to_vec_pretty(&all)?)?;
    std::fs::rename(&written, &file)?;
    Ok(())
}

/// Whether the board was seen on the port, by its name and USB details.
fn same_port(seen: &Seen, port: &str, usb: &UsbPortInfo) -> bool {
    seen.port == port && seen.vid == usb.vid && seen.pid == usb.pid && seen.serial_number == usb.serial_number
}

/// What the port's board probably is, from the boards seen: the one with its ID, if it says it, or else the one last
/// identified on it.
pub fn guess(port: &Port, seen: &[Seen]) -> Option<Guess> {
    if let Some(id) = port.id() {
        return seen.iter().find(|seen| seen.id == id).cloned().map(Guess::Known);
    }
    seen.iter().filter(|seen| same_port(seen, &port.name, &port.usb)).max_by_key(|seen| seen.at).cloned().map(Guess::Maybe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb(vid: u16, serial: Option<&str>) -> UsbPortInfo {
        UsbPortInfo { vid, pid: 1, serial_number: serial.map(str::to_string), manufacturer: None, product: None }
    }

    fn seen(id: &str, port: &str, vid: u16, serial: Option<&str>, at: u64) -> Seen {
        Seen {
            id: id.to_string(),
            chip: "esp32".to_string(),
            board: Some("cyd-esp32-28".to_string()),
            firmware: Some("marco-sparko-capture".to_string()),
            version: Some("0.1.0".to_string()),
            port: port.to_string(),
            vid,
            pid: 1,
            serial_number: serial.map(str::to_string),
            at,
        }
    }

    #[test]
    fn guesses() {
        let all = vec![
            seen("98a316bb08c4", "/dev/cu.usbmodem1", 0x303a, Some("98:A3:16:BB:08:C4"), 1),
            seen("f4650ba9e5c4", "/dev/cu.usbserial-120", 0x1a86, None, 1),
            seen("000000000001", "/dev/cu.usbserial-120", 0x1a86, None, 2),
        ];
        // A board on its chip's own USB is known by its ID, wherever it is plugged in.
        let own = Port { name: "/dev/cu.usbmodem9".to_string(), usb: usb(0x303a, Some("98:A3:16:BB:08:C4")) };
        assert!(matches!(guess(&own, &all), Some(Guess::Known(seen)) if seen.id == "98a316bb08c4"));
        // One on a USB serial chip is maybe the last seen on its port.
        let chip = Port { name: "/dev/cu.usbserial-120".to_string(), usb: usb(0x1a86, None) };
        assert!(matches!(guess(&chip, &all), Some(Guess::Maybe(seen)) if seen.id == "000000000001"));
        let elsewhere = Port { name: "/dev/cu.usbserial-130".to_string(), usb: usb(0x1a86, None) };
        assert_eq!(guess(&elsewhere, &all), None);
        assert_eq!(Guess::Maybe(all[1].clone()).describe(),
            "Maybe an ESP32-2432S028R \"Cheap Yellow Display\", running marco-sparko-capture 0.1.0, ID f4650ba9e5c4");
    }
}
