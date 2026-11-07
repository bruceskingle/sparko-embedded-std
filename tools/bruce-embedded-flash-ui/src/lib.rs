//! A Dioxus component, Flasher, which lists the boards on USB, and flashes firmware built on the platform to one,
//! only if it was built for that board, as the user confirms, see bruce-embedded-flash, for an app's own page, e.g.
//! Marco Sparko's Devices, and for this crate's desktop app.
//!
//! The ports are listed without touching the boards on them. A board plugged in while the component is shown is
//! identified at once, which restarts it, as it has only just started; any other is identified only when the user
//! asks, as it may be busy, e.g. a capture device capturing. Before flashing a board whose firmware does not say it
//! is the firmware's board, the user is asked whether it is, by the board's full name, and offered to restart it, to
//! see which board it is by its lights or screen.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bruce_embedded_flash::{
    Board, BoardType, Confirmed, Connection, FirmwareRecord, Port, ProgressCallbacks, Verdict, a, board_type,
    board_types_with_chip, flash_board, image_record, ports,
    seen::{self, Seen},
};
use dioxus::prelude::*;

mod get_firmware;
pub use get_firmware::GetFirmware;

/// The component's own look, which the page's styles can override, with the flasher- classes.
pub const STYLE: &str = r#"
.flasher table { border-collapse: collapse; }
.flasher th, .flasher td { padding: 0.2em 0.6em; text-align: left; vertical-align: top; }
.flasher tr.flasher-selected { background: rgba(127, 127, 127, 0.18); }
.flasher tr.flasher-port { cursor: pointer; }
.flasher .flasher-muted { opacity: 0.7; }
.flasher .flasher-error { color: #d03030; }
.flasher .flasher-ok { color: #2e9a4a; }
.flasher .flasher-panel { border: 1px solid rgba(127, 127, 127, 0.4); border-radius: 6px; padding: 0.6em 1em; margin: 0.8em 0; }
.flasher .flasher-confirm { border-color: #d08a20; }
.flasher .flasher-types label { display: block; margin: 0.4em 0; }
.flasher progress { width: 20em; }
.flasher button { margin-right: 0.5em; }
.flasher .flasher-backdrop { position: fixed; inset: 0; z-index: 1000; display: flex; align-items: center; justify-content: center; background: rgba(0, 0, 0, 0.55); }
.flasher .flasher-dialog { background: var(--panel, Canvas); border: 1px solid var(--accent, rgba(127, 127, 127, 0.6)); border-radius: 10px; padding: 1em 1.5em; min-width: 26em; max-width: 90vw; box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5); }
.flasher .flasher-dialog h3 { margin-top: 0; }
.flasher .flasher-dialog progress { width: 100%; height: 1.2em; }
"#;

/// Firmware the user can flash: an ELF file's image, with the record of the board it was built for.
#[derive(Clone)]
pub struct Firmware {
    /// Where it came from, e.g. its file's path, which the user is shown.
    pub label: String,
    pub record: FirmwareRecord,
    pub elf: Arc<Vec<u8>>,
}

impl PartialEq for Firmware {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && Arc::ptr_eq(&self.elf, &other.elf)
    }
}

impl Firmware {
    /// The firmware in the ELF file, which must have a firmware record.
    pub fn load(path: &Path) -> Result<Firmware, String> {
        Self::load_as(path, path.display().to_string())
    }

    /// The firmware in the shared firmware folder, see bruce-embedded-firmware, each labelled with the repository
    /// and version it came from, or why it could not be read.
    pub fn load_local() -> Vec<Result<Firmware, String>> {
        match bruce_embedded_firmware::local() {
            Ok(found) => found.iter().map(|local| Self::load_as(&local.path, local.label())).collect(),
            Err(error) => vec![Err(format!("{error:#}"))],
        }
    }

    fn load_as(path: &Path, label: String) -> Result<Firmware, String> {
        let elf = std::fs::read(path).map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
        let record = image_record(&elf).map_err(|error| format!("{}: {error:#}", path.display()))?;
        Ok(Firmware { label, record, elf: Arc::new(elf) })
    }
}

/// A device the app already knows, by its ID, its Wi-Fi MAC in hex, so that its port can say which it is.
#[derive(Clone, PartialEq)]
pub struct KnownDevice {
    pub id: String,
    pub name: String,
    /// The firmware the app needs it to run, e.g. `marco-sparko-capture`, so that flashing other firmware to it,
    /// which would stop it doing what the app needs, is agreed to first, even if its firmware does not say what it is.
    pub firmware: Option<String>,
}

/// What a port's board was found to be, when identified, or why it could not be.
#[derive(Clone)]
enum Found {
    Board(Board),
    Error(String),
}

/// How far flashing has got, which the flasher's thread sends and the component shows.
#[derive(Clone, Default)]
struct FlashProgress {
    address: u32,
    current: usize,
    total: usize,
    verifying: bool,
    /// Whether the part was already on the board, so was not written again.
    skipped: bool,
}

/// ProgressCallbacks sending each change to the component, which shows it as it comes.
struct SentProgress {
    progress: FlashProgress,
    sender: tokio::sync::mpsc::UnboundedSender<FlashProgress>,
}

impl SentProgress {
    fn send(&self) {
        let _ = self.sender.send(self.progress.clone());
    }
}

impl ProgressCallbacks for SentProgress {
    fn init(&mut self, address: u32, total: usize) {
        self.progress = FlashProgress { address, current: 0, total: total.max(1), verifying: false, skipped: false };
        self.send();
    }

    fn update(&mut self, current: usize) {
        self.progress.current = current;
        self.send();
    }

    fn verifying(&mut self) {
        self.progress.verifying = true;
        self.send();
    }

    fn finish(&mut self, skipped: bool) {
        self.progress.current = self.progress.total;
        self.progress.skipped = skipped;
        self.send();
    }
}

/// The dialog shown over the page while flashing, and then with how it ended, until closed.
#[derive(Clone, Default)]
struct FlashDialog {
    /// What is being flashed to where.
    title: String,
    progress: FlashProgress,
    /// How it ended, once it has.
    outcome: Option<Result<String, String>>,
}

/// How often the ports are listed, to notice a board plugged in or out.
const PORT_POLL: Duration = Duration::from_secs(2);

/// Lists the boards on USB and flashes one of them with one of the firmware, see the module's description. known
/// names the devices the app already knows, by ID; preferred names the firmware to offer first for a board whose
/// firmware is other, or unknown, e.g. the app's own; baud, if given, is the baud rate to flash at, rather than the
/// port's own, see Port::flash_baud.
#[component]
pub fn Flasher(
    firmware: Vec<Firmware>,
    known: Vec<KnownDevice>,
    preferred: Option<String>,
    baud: Option<u32>,
) -> Element {
    let mut port_list = use_signal(Vec::<Port>::new);
    // The ports there when the component was first shown, so that any other was plugged in since.
    let mut first_ports = use_signal(|| None::<BTreeSet<String>>);
    let mut found = use_signal(BTreeMap::<String, Found>::new);
    // The boards identified before, on this computer, for what each port's board probably is, see seen.
    let mut seen = use_signal(Vec::<Seen>::new);
    let mut selected = use_signal(|| None::<String>);
    let mut busy = use_signal(|| None::<String>);
    let mut status = use_signal(|| None::<Result<String, String>>);
    let mut dialog = use_signal(|| None::<FlashDialog>);

    let identify = move |port: String| {
        spawn(async move {
            busy.set(Some(format!("Identifying the board on {port}")));
            let name = port.clone();
            let result = tokio::task::spawn_blocking(move || Connection::open(&name, None)?.identify()).await;
            let result = match result {
                Ok(Ok(board)) => Found::Board(board),
                Ok(Err(error)) => Found::Error(format!("{error:#}")),
                Err(error) => Found::Error(format!("{error}")),
            };
            found.write().insert(port, result);
            busy.set(None);
        });
    };

    let restart = move |port: String| {
        spawn(async move {
            busy.set(Some(format!("Restarting the board on {port}")));
            let name = port.clone();
            let result = tokio::task::spawn_blocking(move || Connection::open(&name, None)?.reset()).await;
            status.set(Some(match result {
                Ok(Ok(())) => Ok(format!("Restarted the board on {port}")),
                Ok(Err(error)) => Err(format!("{error:#}")),
                Err(error) => Err(format!("{error}")),
            }));
            busy.set(None);
        });
    };

    // Lists the ports now and every PORT_POLL, and identifies a board plugged in since the first listing.
    use_future(move || async move {
        loop {
            if let Ok(remembered) = tokio::task::spawn_blocking(seen::load).await {
                seen.set(remembered);
            }
            if let Ok(Ok(listed)) = tokio::task::spawn_blocking(ports).await {
                let names: BTreeSet<String> = listed.iter().map(|port| port.name.clone()).collect();
                if first_ports.peek().is_none() {
                    first_ports.set(Some(names.clone()));
                }
                let before: BTreeSet<String> = port_list.peek().iter().map(|port| port.name.clone()).collect();
                let first = first_ports.peek().clone().unwrap_or_default();
                found.write().retain(|name, _| names.contains(name));
                port_list.set(listed);
                for name in names.difference(&before).filter(|name| !first.contains(*name)) {
                    if busy.peek().is_none() {
                        selected.set(Some(name.clone()));
                        identify(name.clone());
                    }
                }
            }
            tokio::time::sleep(PORT_POLL).await;
        }
    });

    let known_name = |id: &str| known.iter().find(|device| device.id == id).map(|device| device.name.clone());
    let selected_port = selected();
    let selected_found = selected_port.as_ref().and_then(|port| found.read().get(port).cloned());
    let is_busy = busy().is_some();

    rsx! {
        div { class: "flasher",
            style { {STYLE} }
            if port_list.read().is_empty() {
                p { "No boards are plugged in by USB." }
            } else {
                table {
                    tr {
                        th { "Port" }
                        th { "What the computer says" }
                        th { "ID" }
                        th { "Board" }
                        th {}
                    }
                    for port in port_list.read().iter().cloned() {
                        {
                            let name = port.name.clone();
                            let board = found.read().get(&name).cloned();
                            let id = match &board {
                                Some(Found::Board(board)) => Some(board.id.clone()),
                                _ => port.id(),
                            };
                            let known_as = id.as_deref().and_then(known_name);
                            let is_selected = selected_port.as_deref() == Some(name.as_str());
                            let (identify_name, restart_name, select_name) = (name.clone(), name.clone(), name.clone());
                            rsx! {
                                tr {
                                    key: "{name}",
                                    class: if is_selected { "flasher-port flasher-selected" } else { "flasher-port" },
                                    onclick: move |_| {
                                        selected.set(Some(select_name.clone()));
                                        status.set(None);
                                    },
                                    td { "{name}" }
                                    td { class: "flasher-muted", "{port.describe()}" }
                                    td {
                                        {id.clone().unwrap_or_default()}
                                        if let Some(known_as) = known_as {
                                            " ({known_as})"
                                        }
                                    }
                                    td {
                                        match board {
                                            Some(Found::Board(board)) => rsx! { {describe_firmware(&board)} },
                                            Some(Found::Error(error)) => rsx! { span { class: "flasher-error", title: "{error}", "Not identified" } },
                                            None => match seen::guess(&port, &seen.read()) {
                                                Some(guess) => rsx! { span { class: "flasher-muted", "{guess.describe()}" } },
                                                None => rsx! { span { class: "flasher-muted", "Not identified yet" } },
                                            },
                                        }
                                    }
                                    td {
                                        button {
                                            disabled: is_busy,
                                            title: "Reads which chip the board is, and which board its firmware was built for, which restarts it",
                                            onclick: move |event| {
                                                event.stop_propagation();
                                                selected.set(Some(identify_name.clone()));
                                                identify(identify_name.clone());
                                            },
                                            "Identify"
                                        }
                                        button {
                                            disabled: is_busy,
                                            title: "Restarts the board, to see which one it is by its lights or screen",
                                            onclick: move |event| {
                                                event.stop_propagation();
                                                restart(restart_name.clone());
                                            },
                                            "Restart"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                p { class: "flasher-muted",
                    "Identifying or restarting a board restarts it. A board plugged in while this is shown is identified at once."
                }
            }
            if let Some(what) = busy() {
                p { "{what}…" }
            }
            if let (Some(port), Some(Found::Error(error))) = (selected_port.clone(), selected_found.clone()) {
                div { class: "flasher-panel",
                    p { class: "flasher-error", "The board on {port} could not be identified: {error}" }
                    p {
                        "If it does not start its bootloader when asked over USB, start it by hand, then Identify it again:"
                    }
                    ul {
                        for board_type in bruce_embedded_flash::BOARD_TYPES.iter() {
                            li { b { "{board_type.name}" } ": {board_type.boot}" }
                        }
                    }
                }
            }
            if let (Some(port), Some(Found::Board(board))) = (selected_port.clone(), selected_found.clone()) {
                Chosen {
                    key: "{port}-{board.id}",
                    port,
                    board,
                    firmware: firmware.clone(),
                    known: known.clone(),
                    preferred: preferred.clone(),
                    busy: is_busy,
                    on_restart: move |port: String| restart(port),
                    on_flash: move |(port, board, image, confirmed): (String, Board, Firmware, Confirmed)| {
                        spawn(async move {
                            busy.set(Some(format!("Flashing {} to the board on {port}", image.record.firmware())));
                            status.set(None);
                            dialog.set(Some(FlashDialog {
                                title: format!("Flashing {} {} to the board on {port}", image.record.firmware(),
                                    image.record.version()),
                                ..FlashDialog::default()
                            }));
                            let name = port.clone();
                            let elf = image.elf.clone();
                            let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
                            let id = board.id.clone();
                            let baud = baud.or_else(|| {
                                port_list.peek().iter().find(|found| found.name == port).and_then(Port::flash_baud)
                            });
                            let mut flashing = tokio::task::spawn_blocking(move || {
                                let mut progress = SentProgress { progress: FlashProgress::default(), sender };
                                flash_board(&name, baud, &elf, &id, &confirmed, &mut progress)
                            });
                            // Shows each change as it comes, until flashing ends, noting whether any part was written,
                            // as none is when the firmware is already on the board.
                            let mut wrote = false;
                            let result = loop {
                                tokio::select! {
                                    Some(update) = receiver.recv() => {
                                        wrote |= update.total > 0 && !update.skipped && update.current < update.total;
                                        if let Some(shown) = dialog.write().as_mut() {
                                            shown.progress = update;
                                        }
                                    }
                                    result = &mut flashing => break result,
                                }
                            };
                            let outcome = match result {
                                Ok(Ok(_)) if wrote => Ok(format!("Flashed {} {} to the board on {port}, and restarted it",
                                    image.record.firmware(), image.record.version())),
                                Ok(Ok(_)) => Ok(format!("{} {} was already on the board on {port}, so nothing was \
                                    written; restarted it", image.record.firmware(), image.record.version())),
                                Ok(Err(error)) => Err(format!("{error:#}")),
                                Err(error) => Err(format!("{error}")),
                            };
                            if let Some(shown) = dialog.write().as_mut() {
                                shown.outcome = Some(outcome.clone());
                            }
                            status.set(Some(outcome));
                            busy.set(None);
                            identify(port);
                        });
                    },
                }
            }
            match status() {
                Some(Ok(message)) => rsx! { p { class: "flasher-ok", "{message}" } },
                Some(Err(message)) => rsx! { p { class: "flasher-error", "{message}" } },
                None => rsx! {},
            }
            // Over the page while flashing, as the page's own lines are easily missed, and then with how it ended.
            if let Some(shown) = dialog() {
                div { class: "flasher-backdrop",
                    div { class: "flasher-dialog", role: "dialog",
                        h3 { "{shown.title}" }
                        match shown.outcome.clone() {
                            None => rsx! {
                                p { {describe_progress(&shown.progress)} }
                                if shown.progress.total > 0 && !shown.progress.skipped {
                                    progress { max: "{shown.progress.total}", value: "{shown.progress.current}" }
                                }
                                p { class: "flasher-muted", "Leave the board plugged in until this is done." }
                            },
                            Some(Ok(message)) => rsx! {
                                p { class: "flasher-ok", "{message}" }
                                button { onclick: move |_| dialog.set(None), "Close" }
                            },
                            Some(Err(message)) => rsx! {
                                p { class: "flasher-error", "{message}" }
                                button { onclick: move |_| dialog.set(None), "Close" }
                            },
                        }
                    }
                }
            }
        }
    }
}

/// What flashing is doing, by the part of the flash being written, as the bar alone says nothing while the chip
/// erases the space for a part before writing it, which takes a while for the app on a slow board.
fn describe_progress(shown: &FlashProgress) -> String {
    if shown.total == 0 {
        return "Connecting to the board's bootloader…".to_string();
    }
    let part = match shown.address {
        0x0 | 0x1000 => "the bootloader".to_string(),
        0x8000 => "the partition table".to_string(),
        address => format!("the firmware, at 0x{address:x}"),
    };
    if shown.skipped {
        format!("Not writing {part}, which is already on the board")
    } else if shown.verifying {
        format!("Checking {part}")
    } else if shown.current == 0 {
        format!("Erasing the space for {part}")
    } else {
        format!("Writing {part}, {}%", shown.current * 100 / shown.total)
    }
}

/// What the board's firmware says it was built for, by the board's full name.
fn describe_firmware(board: &Board) -> String {
    let records: Vec<&FirmwareRecord> = board.apps.iter().filter_map(|app| app.record.as_ref()).collect();
    match records.first() {
        Some(record) => format!("{}, {} {}", full_name(record.board()), record.firmware(), record.version()),
        None if board.apps.is_empty() => format!("An {} with no firmware", board.chip),
        None => format!("An {} whose firmware does not say which board it is", board.chip),
    }
}

/// The firmware the board runs, by name and version, as its records say.
fn running(board: &Board) -> String {
    let names: Vec<String> = board
        .apps
        .iter()
        .filter_map(|app| app.record.as_ref())
        .map(|record| format!("{} {}", record.firmware(), record.version()))
        .collect();
    names.join(" and ")
}

/// The board's full name, e.g. "Seeed Studio XIAO ESP32C6", or its id if it is not one this knows.
fn full_name(id: &str) -> String {
    board_type(id).map_or(id.to_string(), |board_type| board_type.name.to_string())
}

/// The firmware for a board, newest first, by its version's numbers.
fn firmware_for<'a>(firmware: &'a [Firmware], board: &str) -> Vec<&'a Firmware> {
    let mut found: Vec<&Firmware> = firmware.iter().filter(|image| image.record.board() == board).collect();
    found.sort_by(|a, b| version_numbers(b.record.version()).cmp(&version_numbers(a.record.version())));
    found
}

/// A version's numbers, e.g. [0, 5, 1] for 0.5.1, for ordering versions.
fn version_numbers(version: &str) -> Vec<u64> {
    version.split(|c: char| !c.is_ascii_digit()).filter_map(|part| part.parse().ok()).collect()
}

/// The firmware to offer first for a board: the newest of the firmware it runs, as its record says, or of the
/// firmware the app needs a device it knows to run, or of the preferred firmware, or else the newest for the board.
fn default_firmware(
    firmware: &[Firmware],
    board_type: &str,
    board: &Board,
    known: &[KnownDevice],
    preferred: Option<&str>,
) -> Option<Firmware> {
    let candidates = firmware_for(firmware, board_type);
    let running = board.apps.iter().filter_map(|app| app.record.as_ref()).map(|record| record.firmware().to_string());
    let needed = known.iter().find(|device| device.id == board.id).and_then(|device| device.firmware.clone());
    running
        .chain(needed)
        .chain(preferred.map(str::to_string))
        .find_map(|name| candidates.iter().find(|image| image.record.firmware() == name))
        .or(candidates.first())
        .map(|image| (*image).clone())
}

/// The identified board on the selected port. If its firmware says which board it is, it says so, and offers Flash,
/// with the firmware it runs, the newest, and Other Firmware, for the rest with its chip; if not, the user picks which
/// board it is, from those with its chip, by their full names. Flash asks for one more click, to confirm, if the
/// firmware is for another board than its firmware says, or if the board's firmware does not say, offering to
/// restart it, to see which board it is, or if the firmware would replace other firmware, e.g. a demo over a capture
/// device's.
#[component]
fn Chosen(
    port: String,
    board: Board,
    firmware: Vec<Firmware>,
    known: Vec<KnownDevice>,
    preferred: Option<String>,
    busy: bool,
    on_restart: EventHandler<String>,
    on_flash: EventHandler<(String, Board, Firmware, Confirmed)>,
) -> Element {
    // The board the user picked, when its firmware does not say.
    let mut picked_type = use_signal(|| None::<&'static str>);
    // Whether the user asked for other firmware, and which they picked, by its label.
    let mut other = use_signal(|| false);
    let mut picked_firmware = use_signal(|| None::<String>);
    // Whether Flash was clicked for firmware which needs confirming.
    let mut confirming = use_signal(|| false);

    let says = board.board_type();
    let board_type = says.or_else(|| picked_type().and_then(board_type));
    let default = board_type
        .and_then(|board_type| default_firmware(&firmware, board_type.id, &board, &known, preferred.as_deref()));
    // All the firmware for the chip, for Other Firmware, newest first for each board.
    let for_chip: Vec<Firmware> = board_types_with_chip(&board.chip)
        .flat_map(|board_type| firmware_for(&firmware, board_type.id).into_iter().cloned().collect::<Vec<_>>())
        .collect();
    let image = if other() {
        picked_firmware().and_then(|label| for_chip.iter().find(|image| image.label == label).cloned()).or(default.clone())
    } else {
        default.clone()
    };

    // What flashing the image needs confirming, as sentences, and what the user confirms by clicking.
    let mut warnings = Vec::new();
    let mut confirmed = Confirmed::default();
    let mut unknown_board = false;
    if let Some(image) = &image {
        let image_name = full_name(image.record.board());
        match board.verdict(&image.record) {
            Verdict::Same => {}
            Verdict::Different(other) => {
                warnings.push(format!("Its firmware says this board is {}, but {} is for {}. Firmware for the wrong \
                    board can damage it.", a(&full_name(&other)), image.record.firmware(), a(&image_name)));
                confirmed.board = Some(image.record.board().to_string());
            }
            Verdict::Unknown => {
                let looks = board_type.map(|board_type| board_type.looks).unwrap_or_default();
                warnings.push(format!("Its firmware does not say which board it is. Is it {}? {looks} Firmware for \
                    the wrong board can damage it.", a(&image_name)));
                confirmed.board = Some(image.record.board().to_string());
                unknown_board = true;
            }
        }
        let device = known.iter().find(|device| device.id == board.id);
        if let Some(replaced) = board.replaced_firmware(&image.record) {
            warnings.push(format!("It runs {replaced}, which {} would replace, so it would stop doing what it does.",
                image.record.firmware()));
            confirmed.replace_firmware = true;
        } else if let Some((needed, name)) = device
            .and_then(|device| device.firmware.clone().map(|needed| (needed, device.name.clone())))
            .filter(|(needed, _)| needed != image.record.firmware())
        {
            warnings.push(format!("It is a device the app knows, {name}, which needs {needed}, which {} would \
                replace.", image.record.firmware()));
            confirmed.replace_firmware = true;
        }
    }

    let flash = {
        let (port, board, image, confirmed) = (port.clone(), board.clone(), image.clone(), confirmed.clone());
        move |_| {
            confirming.set(false);
            if let Some(image) = image.clone() {
                on_flash.call((port.clone(), board.clone(), image, confirmed.clone()));
            }
        }
    };
    let needs_confirming = !warnings.is_empty();
    let restart_port = port.clone();

    rsx! {
        div { class: "flasher-panel",
            p {
                "The board on {port} is an {board.chip}"
                if let Some(revision) = &board.revision {
                    " {revision}"
                }
                ", ID {board.id}, with {board.flash_size} of flash. "
                match says {
                    Some(says) => rsx! { b { "It is {a(says.name)}" } ", as its firmware says, running {running(&board)}." },
                    None => rsx! { "{describe_firmware(&board)}." },
                }
            }
            if says.is_none() {
                {
                    let types: Vec<&'static BoardType> = board_types_with_chip(&board.chip).collect();
                    rsx! {
                        if types.is_empty() {
                            p { class: "flasher-error", "No supported board has an {board.chip}." }
                        } else {
                            p { "Which board is it?" }
                            div { class: "flasher-types",
                                for board_type in types.iter().copied() {
                                    {
                                        let has_firmware = !firmware_for(&firmware, board_type.id).is_empty();
                                        let id = board_type.id;
                                        rsx! {
                                            label {
                                                key: "{id}",
                                                class: if has_firmware { "" } else { "flasher-muted" },
                                                input {
                                                    r#type: "radio",
                                                    name: "flasher-board-type",
                                                    checked: picked_type() == Some(id),
                                                    disabled: busy,
                                                    onchange: move |_| {
                                                        picked_type.set(Some(id));
                                                        confirming.set(false);
                                                    },
                                                }
                                                " {board_type.name}"
                                                if !has_firmware {
                                                    " — no firmware for it here"
                                                }
                                                br {}
                                                span { class: "flasher-muted", "{board_type.looks} " }
                                                if let Some(url) = board_type.url {
                                                    a { href: "{url}", target: "_blank", "Maker's page" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if other() {
                p { "Which firmware?" }
                div { class: "flasher-types",
                    for candidate in for_chip.iter().cloned() {
                        {
                            let label = candidate.label.clone();
                            let checked = image.as_ref().map(|image| &image.label) == Some(&label);
                            rsx! {
                                label {
                                    key: "{label}",
                                    input {
                                        r#type: "radio",
                                        name: "flasher-firmware",
                                        checked,
                                        disabled: busy,
                                        onchange: move |_| {
                                            picked_firmware.set(Some(label.clone()));
                                            confirming.set(false);
                                        },
                                    }
                                    " {candidate.record.firmware()} {candidate.record.version()}, for {a(&full_name(candidate.record.board()))}"
                                    br {}
                                    span { class: "flasher-muted", "{candidate.label}" }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(image) = image.clone() {
                if confirming() {
                    div { class: "flasher-panel flasher-confirm",
                        for warning in warnings.iter() {
                            p { "{warning}" }
                        }
                        p { b { "Flash {image.record.firmware()} {image.record.version()} to the board on {port}?" } }
                        button { disabled: busy, onclick: flash.clone(), "Yes, Flash It" }
                        if unknown_board {
                            button {
                                disabled: busy,
                                onclick: move |_| on_restart.call(restart_port.clone()),
                                "Restart It, to See Which Board It Is"
                            }
                        }
                        button { disabled: busy, onclick: move |_| confirming.set(false), "Cancel" }
                    }
                } else {
                    p {
                        button {
                            disabled: busy,
                            onclick: {
                                let mut flash = flash.clone();
                                move |event| {
                                    if needs_confirming {
                                        confirming.set(true);
                                    } else {
                                        flash(event);
                                    }
                                }
                            },
                            "Flash {image.record.firmware()} {image.record.version()}"
                        }
                        if !other() && for_chip.len() > 1 {
                            button { disabled: busy, onclick: move |_| other.set(true), "Other Firmware" }
                        }
                        if other() {
                            button {
                                disabled: busy,
                                onclick: move |_| {
                                    other.set(false);
                                    picked_firmware.set(None);
                                },
                                "Back"
                            }
                        }
                    }
                    p { class: "flasher-muted", "From {image.label}" }
                }
            } else if let Some(board_type) = board_type {
                p { class: "flasher-muted", "There is no firmware for {a(board_type.name)} here." }
                if !other() && !for_chip.is_empty() {
                    button { disabled: busy, onclick: move |_| other.set(true), "Other Firmware" }
                }
            }
        }
    }
}
