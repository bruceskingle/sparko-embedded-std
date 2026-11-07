//! Finding boards on USB, and flashing firmware built on the platform to one only if it was built for that board,
//! from the firmware record in the image to be flashed and in the firmware already on the board, see
//! bruce-embedded-firmware-record, all in this process, with espflash's library, so that an app can flash boards on
//! a computer with no development tools.
//!
//! A board whose firmware has no record, e.g. a new board, or one last flashed before records, is unknown, and the
//! user is asked to say which board it is; one whose firmware was built for another board is refused unless the user
//! says, explicitly, which board it is. A wrong board's firmware drives the wrong pins, and once broke a board's
//! display.

use std::fs;
use std::time::Duration;

pub mod seen;

use anyhow::{Context, anyhow, bail};
pub use bruce_embedded_firmware_record::{BOARD_TYPES, BoardType, FirmwareRecord, board_type, board_types_with_chip};
use espflash::connection::{Connection as EspConnection, ResetAfterOperation, ResetBeforeOperation};
use espflash::flasher::{FlashData, FlashSettings, FlashSize, Flasher};
use espflash::image_format::idf::IdfBootloaderFormat;
pub use espflash::target::{DefaultProgressCallback, ProgressCallbacks};
use object::{Object, ObjectSection};
use serialport::{FlowControl, SerialPortType, UsbPortInfo};

/// The name with "a" or "an" before it, as it starts with a vowel or not, e.g. "an ESP32 DevKit V1", "a Seeed Studio
/// XIAO ESP32C6", for a board's name in a sentence.
pub fn a(name: &str) -> String {
    let vowel = name.chars().next().is_some_and(|first| "aeiouAEIOU".contains(first));
    format!("{} {name}", if vowel { "an" } else { "a" })
}

/// Where ESP-IDF's partition table is, as espflash and ESP-IDF write it by default.
pub const PARTITION_TABLE_OFFSET: u32 = 0x8000;

/// How much of the partition table to read, its largest.
const PARTITION_TABLE_SIZE: u32 = 0xc00;

/// How much of the start of an app partition to read, which holds the image's header, ESP-IDF's app descriptor and
/// then the firmware record.
const APP_HEAD_SIZE: u32 = 0x1000;

/// The size of the blocks the board's flash is read in, see Connection::read_flash.
const READ_BLOCK: u32 = 0x400;

/// Espressif's USB vendor ID, which a chip's own USB port, e.g. an ESP32-S3's or -C6's, has.
const ESPRESSIF_VID: u16 = 0x303a;

/// The record of the board the image in an ELF file was built for, from the section the linker puts it in.
pub fn image_record(elf: &[u8]) -> anyhow::Result<FirmwareRecord> {
    let file = object::File::parse(elf).context("Unable to read the firmware's ELF file")?;
    let section = file
        .section_by_name(".flash.appdesc")
        .ok_or_else(|| anyhow!("The firmware has no app descriptor section, so is not ESP-IDF firmware"))?;
    let data = section.data().context("Unable to read the firmware's app descriptor section")?;
    FirmwareRecord::find(data).ok_or_else(|| {
        anyhow!("The firmware has no firmware record, so does not say which board it is for: build it with \
            with_firmware(sparko_esp_idf::firmware!())")
    })
}

/// A USB serial port, as the computer says it is, without touching the board on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    /// e.g. /dev/cu.usbmodem14301.
    pub name: String,
    pub usb: UsbPortInfo,
}

impl Port {
    /// The device's ID, its Wi-Fi MAC in hex, as the platform's API says it, if the port is the chip's own USB, e.g.
    /// an ESP32-S3's or -C6's, whose serial number is its MAC, so that a board can be recognised without restarting
    /// it. A board with a USB serial chip, e.g. a CYD, does not say.
    pub fn id(&self) -> Option<String> {
        let serial = self.usb.serial_number.as_deref()?;
        let id = serial.replace(':', "").to_lowercase();
        (self.usb.vid == ESPRESSIF_VID && id.len() == 12 && id.chars().all(|c| c.is_ascii_hexdigit())).then_some(id)
    }

    /// The baud rate to flash at: None, the default, for the chip's own USB, which ignores it, and 460800, 4 times
    /// the default, for a USB serial chip, e.g. a CYD's, which flashes slowly at the default, but some of which fail
    /// at 921600.
    pub fn flash_baud(&self) -> Option<u32> {
        (self.usb.vid != ESPRESSIF_VID).then_some(460_800)
    }

    /// What is on the port, e.g. "Espressif USB JTAG/serial debug unit" or a USB serial chip's name.
    pub fn describe(&self) -> String {
        let usb = &self.usb;
        let what = [usb.manufacturer.as_deref(), usb.product.as_deref()].into_iter().flatten().collect::<Vec<_>>();
        let what = if what.is_empty() { "USB serial port".to_string() } else { what.join(" ") };
        format!("{what} ({:04x}:{:04x})", usb.vid, usb.pid)
    }
}

/// The USB serial ports, which may have boards on them, found without touching them. On macOS each port is listed as
/// a cu. and a tty. device, of which only the cu. one is kept, as it is the one to open.
pub fn ports() -> anyhow::Result<Vec<Port>> {
    let mut ports: Vec<Port> = serialport::available_ports()
        .context("Unable to list the serial ports")?
        .into_iter()
        .filter(|port| !(cfg!(target_os = "macos") && port.port_name.starts_with("/dev/tty.")))
        .filter_map(|port| match port.port_type {
            SerialPortType::UsbPort(usb) => Some(Port { name: port.port_name, usb }),
            _ => None,
        })
        .collect();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ports)
}

/// The port of the only board plugged in, or an error naming them all if there are several, as which is which cannot
/// be told from their names.
pub fn only_port() -> anyhow::Result<String> {
    let ports = ports()?;
    match ports.as_slice() {
        [port] => Ok(port.name.clone()),
        [] => bail!("No board is plugged in"),
        _ => bail!(
            "Several boards are plugged in, say which with --port: {}",
            ports.iter().map(|port| port.name.as_str()).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// An app partition on a board, and the record of the firmware in it, if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub label: String,
    pub offset: u32,
    pub record: Option<FirmwareRecord>,
}

/// A board on a serial port, as its ROM bootloader and flash say it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub port: String,
    /// The chip, as espflash names it, e.g. `esp32s3`, as BoardType::chip.
    pub chip: String,
    /// e.g. `v0.2`.
    pub revision: Option<String>,
    /// The device's ID, its Wi-Fi MAC, in hex, as the platform's API says it.
    pub id: String,
    pub flash_size: String,
    /// The app partitions, empty if the board has no partition table, e.g. a new board.
    pub apps: Vec<App>,
}

/// Whether an image may be flashed to a board, from their records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The board's firmware was built for the image's board.
    Same,
    /// The board's firmware has no record, so the user must say which board it is.
    Unknown,
    /// The board's firmware was built for another board, named.
    Different(String),
}

impl Board {
    /// Whether the image, of which this is the record, may be flashed to the board.
    pub fn verdict(&self, image: &FirmwareRecord) -> Verdict {
        let mut boards = self.apps.iter().filter_map(|app| app.record.as_ref()).map(FirmwareRecord::board);
        match boards.find(|board| *board != image.board()) {
            Some(other) => Verdict::Different(other.to_string()),
            None if self.apps.iter().any(|app| app.record.is_some()) => Verdict::Same,
            None => Verdict::Unknown,
        }
    }

    /// The board its firmware says it is, if its records say one board.
    pub fn board_type(&self) -> Option<&'static BoardType> {
        let mut records = self.apps.iter().filter_map(|app| app.record.as_ref());
        let first = records.next()?;
        records.all(|record| record.board() == first.board()).then(|| board_type(first.board())).flatten()
    }

    /// The firmware on the board, by name, if its records name firmware other than the image's, e.g. a board's demo
    /// over a capture device's firmware, which the user must agree to replace, as the board would stop doing what it
    /// did.
    pub fn replaced_firmware(&self, image: &FirmwareRecord) -> Option<String> {
        self.apps
            .iter()
            .filter_map(|app| app.record.as_ref())
            .find(|record| record.firmware() != image.firmware())
            .map(|record| record.firmware().to_string())
    }

    /// An error unless the image's board has this board's chip, which no confirmation can override, as firmware for
    /// another chip cannot run.
    pub fn check_chip(&self, image: &FirmwareRecord) -> anyhow::Result<()> {
        let board_type = board_type(image.board())
            .ok_or_else(|| anyhow!("The firmware is for a {}, which is not a board this knows", image.board()))?;
        if board_type.chip != self.chip {
            bail!("The firmware is for a {}, an {}, but the board is an {}", board_type.name, board_type.chip,
                self.chip);
        }
        Ok(())
    }
}

/// The app partitions in a partition table, as ESP-IDF lays it out: 32 byte entries, each starting 0xaa 0x50, then
/// its type, 0 for an app, subtype, offset, size, and label, ended by an MD5 entry or erased flash.
pub fn app_partitions(table: &[u8]) -> Vec<(String, u32)> {
    table
        .chunks_exact(32)
        .take_while(|entry| entry[0] == 0xaa && entry[1] == 0x50)
        .filter(|entry| entry[2] == 0)
        .map(|entry| {
            let offset = u32::from_le_bytes(entry[4..8].try_into().unwrap());
            let label = &entry[12..28];
            let end = label.iter().position(|&b| b == 0).unwrap_or(label.len());
            (String::from_utf8_lossy(&label[..end]).into_owned(), offset)
        })
        .collect()
}

/// A connection to a board's ROM bootloader, through espflash's flasher stub, made by restarting the board into it.
/// The board stays in its bootloader until reset, which closing the connection does, however it ends, e.g. after a
/// refusal or an error, so that the board is not left stopped.
pub struct Connection {
    port: String,
    /// The port's USB details, for remembering the board, see seen.
    usb: UsbPortInfo,
    flasher: Option<Flasher>,
}

impl Connection {
    /// Restarts the board on the port into its bootloader and connects to it, then, if given a baud rate, changes to
    /// it, e.g. 460800, 4 times faster than the default, but which some boards' USB serial chips cannot do.
    pub fn open(port: &str, baud: Option<u32>) -> anyhow::Result<Self> {
        let usb = ports()?
            .into_iter()
            .find(|candidate| candidate.name == port)
            .map(|candidate| candidate.usb)
            .unwrap_or(UsbPortInfo { vid: 0, pid: 0, serial_number: None, manufacturer: None, product: None });
        let serial = serialport::new(port, 115_200)
            .flow_control(FlowControl::None)
            .open_native()
            .with_context(|| format!("Unable to open {port}"))?;
        let connection = EspConnection::new(serial, usb.clone(), ResetAfterOperation::HardReset,
            ResetBeforeOperation::DefaultReset, 115_200);
        let flasher = Flasher::connect(connection, true, true, true, None, baud).map_err(|error| {
            anyhow!("Unable to connect to the board's bootloader on {port}: {error}. If it does not start its \
                bootloader when asked over USB, start it by hand, e.g. hold BOOT and press RESET, and try again")
        })?;
        Ok(Connection { port: port.to_string(), usb, flasher: Some(flasher) })
    }

    /// Reads the board's chip, ID and firmware records, and remembers them, see seen.
    pub fn identify(&mut self) -> anyhow::Result<Board> {
        let board = self.read_board()?;
        if let Err(error) = seen::remember(&board, &self.usb) {
            log::warn!("Unable to remember the board, {error:#}");
        }
        Ok(board)
    }

    fn read_board(&mut self) -> anyhow::Result<Board> {
        let info = self.flasher().device_info().map_err(|error| anyhow!("Unable to read the board's chip: {error}"))?;
        let id = info
            .mac_address
            .ok_or_else(|| anyhow!("The board did not say its MAC address"))?
            .replace(':', "")
            .to_lowercase();
        let table = self.read_flash(PARTITION_TABLE_OFFSET, PARTITION_TABLE_SIZE)?;
        let mut apps = Vec::new();
        for (label, offset) in app_partitions(&table) {
            let head = self.read_flash(offset, APP_HEAD_SIZE)?;
            apps.push(App { label, offset, record: FirmwareRecord::find(&head) });
        }
        Ok(Board {
            port: self.port.clone(),
            chip: info.chip.to_string(),
            revision: info.revision.map(|(major, minor)| format!("v{major}.{minor}")),
            id,
            flash_size: info.flash_size.to_string(),
            apps,
        })
    }

    /// Flashes the ELF file's image, with ESP-IDF's bootloader and partition table as espflash makes them, as
    /// `espflash flash` does, after the caller has checked that it may, see Board::verdict and Board::check_chip.
    pub fn flash(&mut self, elf: &[u8], progress: &mut dyn ProgressCallbacks) -> anyhow::Result<()> {
        let chip = self.flasher().chip();
        let crystal = chip
            .xtal_frequency(self.flasher().connection())
            .map_err(|error| anyhow!("Unable to read the board's crystal frequency: {error}"))?;
        let flash_size = self.flasher().flash_detect().ok().flatten().unwrap_or_else(FlashSize::default);
        self.flasher().set_flash_size(flash_size);
        let flash_data = FlashData::new(FlashSettings::new(None, Some(flash_size), None), 0, None, chip, crystal);
        let image = IdfBootloaderFormat::new(elf, &flash_data, None, None, None, None)
            .map_err(|error| anyhow!("Unable to make the image to flash: {error}"))?;
        self.flasher().load_image_to_flash(progress, image.into()).map_err(|error| anyhow!("Flashing failed: {error}"))
    }

    /// Restarts the board, to run its firmware, and closes the connection.
    pub fn reset(mut self) -> anyhow::Result<()> {
        self.restart()
    }

    /// Restarts the board, once.
    fn restart(&mut self) -> anyhow::Result<()> {
        let Some(mut flasher) = self.flasher.take() else { return Ok(()) };
        let chip = flasher.chip();
        flasher.connection().reset_after(true, chip).map_err(|error| anyhow!("Unable to restart the board: {error}"))
    }

    /// The flasher, which is there until the connection is closed.
    fn flasher(&mut self) -> &mut Flasher {
        self.flasher.as_mut().expect("the connection is open")
    }

    /// The bytes of the board's flash from the offset, through a file, as espflash reads to one, in small blocks, one
    /// at a time, as some USB serial chips, e.g. a CYD's CH340, drop bytes when the board sends many at once, which
    /// costs little for the few KB read, and again once if it still fails.
    fn read_flash(&mut self, offset: u32, size: u32) -> anyhow::Result<Vec<u8>> {
        let file = std::env::temp_dir().join(format!("bruce-embedded-flash-{}-{offset:x}.bin", std::process::id()));
        let mut result = self.flasher().read_flash(offset, size, READ_BLOCK, 1, file.clone());
        if result.is_err() {
            result = self.flasher().read_flash(offset, size, READ_BLOCK, 1, file.clone());
        }
        result.map_err(|error| anyhow!("Unable to read the board's flash at 0x{offset:x}: {error}"))?;
        let bytes = fs::read(&file).with_context(|| format!("Unable to read {}", file.display()));
        let _ = fs::remove_file(&file);
        bytes
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.restart();
    }
}

/// What the user has agreed to, before flashing, see flash_board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confirmed {
    /// The board the user says it is, by id, which must be the image's board, when the board's firmware does not say
    /// it is that board.
    pub board: Option<String>,
    /// That the user agrees to replace the board's firmware with other firmware, see Board::replaced_firmware.
    pub replace_firmware: bool,
}

/// Flashes the ELF file's image to the board on the port, with every check, so that a caller cannot leave one out:
/// it must be the board the caller identified, by its ID, as the port may now have another; its chip must be the one
/// the image's board has, see Board::check_chip; unless its firmware was built for the image's board, the user must
/// have confirmed that it is that board; and if its firmware is other firmware, the user must have agreed to replace
/// it. Restarts the board afterwards, or on failure, and returns it as it was before flashing.
pub fn flash_board(
    port: &str,
    baud: Option<u32>,
    elf: &[u8],
    expected_id: &str,
    confirmed: &Confirmed,
    progress: &mut dyn ProgressCallbacks,
) -> anyhow::Result<Board> {
    let image = image_record(elf)?;
    let mut connection = Connection::open(port, baud)?;
    let board = connection.identify()?;
    if board.id != expected_id {
        bail!("Not flashing: the board on {port} is now ID {}, not {expected_id}", board.id);
    }
    board.check_chip(&image)?;
    if board.verdict(&image) != Verdict::Same && confirmed.board.as_deref() != Some(image.board()) {
        bail!("Not flashing: the board's firmware does not say it is a {}, and the user has not said so",
            image.board());
    }
    if let Some(replaced) = board.replaced_firmware(&image)
        && !confirmed.replace_firmware
    {
        bail!("Not flashing: the board runs {replaced}, which {} would replace, and the user has not agreed to it",
            image.firmware());
    }
    connection.flash(elf, progress)?;
    connection.reset()?;
    Ok(board)
}

/// Shows what the board on the port writes, e.g. its log, until the process is stopped, without restarting it,
/// waiting for the port to come back if it is the chip's own USB, which goes while the chip restarts. The board resets
/// the chip when DTR and RTS differ, so both are kept set, see marco-sparko's capture/log.py.
pub fn monitor(port: &str, out: &mut dyn std::io::Write) -> anyhow::Result<()> {
    let mut serial = None;
    for _ in 0..50 {
        match serialport::new(port, 115_200).timeout(Duration::from_millis(200)).open_native() {
            Ok(opened) => {
                serial = Some(opened);
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let mut serial = serial.ok_or_else(|| anyhow!("Unable to open {port} to show the board's log"))?;
    use serialport::SerialPort;
    let _ = serial.write_data_terminal_ready(true);
    let _ = serial.write_request_to_send(true);
    let mut buffer = [0u8; 1024];
    loop {
        match std::io::Read::read(&mut serial, &mut buffer) {
            Ok(0) => {}
            Ok(count) => {
                out.write_all(&buffer[..count])?;
                out.flush()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error).context(format!("Unable to read {port}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u8, subtype: u8, offset: u32, label: &str) -> [u8; 32] {
        let mut entry = [0u8; 32];
        entry[0] = 0xaa;
        entry[1] = 0x50;
        entry[2] = kind;
        entry[3] = subtype;
        entry[4..8].copy_from_slice(&offset.to_le_bytes());
        entry[12..12 + label.len()].copy_from_slice(label.as_bytes());
        entry
    }

    #[test]
    fn apps_in_partition_table() {
        let mut table = Vec::new();
        table.extend(entry(1, 2, 0x9000, "nvs"));
        table.extend(entry(0, 0, 0x10000, "factory"));
        table.extend([0xeb; 32]);
        table.extend([0xff; 64]);
        assert_eq!(app_partitions(&table), vec![("factory".to_string(), 0x10000)]);
        assert!(app_partitions(&[0xff; 0xc00]).is_empty());
    }

    fn board(records: &[Option<FirmwareRecord>]) -> Board {
        Board {
            port: "port".to_string(),
            chip: "esp32s3".to_string(),
            revision: None,
            id: "id".to_string(),
            flash_size: "16MB".to_string(),
            apps: records.iter().map(|record| App { label: "app".to_string(), offset: 0, record: record.clone() }).collect(),
        }
    }

    #[test]
    fn verdicts() {
        let rlcd = FirmwareRecord::new("wave-esp32s3-rlcd-42", "marco-sparko-capture", "0.1.0");
        let guition = FirmwareRecord::new("guition-jc3248w535c", "marco-sparko-capture", "0.1.0");
        assert_eq!(board(&[]).verdict(&rlcd), Verdict::Unknown);
        assert_eq!(board(&[None]).verdict(&rlcd), Verdict::Unknown);
        assert_eq!(board(&[Some(rlcd.clone())]).verdict(&rlcd), Verdict::Same);
        assert_eq!(board(&[Some(guition.clone())]).verdict(&rlcd),
            Verdict::Different("guition-jc3248w535c".to_string()));
        assert_eq!(board(&[Some(rlcd.clone()), Some(guition)]).verdict(&rlcd),
            Verdict::Different("guition-jc3248w535c".to_string()));
    }

    #[test]
    fn replaced_firmware() {
        let capture = FirmwareRecord::new("guition-jc3248w535c", "marco-sparko-capture", "0.1.0");
        let newer = FirmwareRecord::new("guition-jc3248w535c", "marco-sparko-capture", "0.2.0");
        let demo = FirmwareRecord::new("guition-jc3248w535c", "demo", "0.1.0");
        assert_eq!(board(&[None]).replaced_firmware(&demo), None);
        assert_eq!(board(&[Some(capture.clone())]).replaced_firmware(&newer), None);
        assert_eq!(board(&[Some(capture)]).replaced_firmware(&demo), Some("marco-sparko-capture".to_string()));
    }

    #[test]
    fn chips() {
        let rlcd = FirmwareRecord::new("wave-esp32s3-rlcd-42", "marco-sparko-capture", "0.1.0");
        let xiao = FirmwareRecord::new("seeed-esp32c6-xiao", "demo", "0.1.0");
        let unknown = FirmwareRecord::new("no-such-board", "demo", "0.1.0");
        assert!(board(&[]).check_chip(&rlcd).is_ok());
        assert!(board(&[]).check_chip(&xiao).is_err());
        assert!(board(&[]).check_chip(&unknown).is_err());
    }
}
