//! The command line: `list` the boards on USB, without touching them, `restart` or `identify` one, `boards` the
//! platform supports, and `flash` firmware to one, only if it was built for that board. It is the cargo runner of the
//! board crates, so `cargo run` checks too.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{Context, bail};
use bruce_embedded_flash::{
    BOARD_TYPES, Board, BoardType, Confirmed, Connection, ProgressCallbacks, Verdict, a, board_type, flash_board, image_record,
    monitor, only_port, ports,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Flashes bruce-embedded-std firmware to a board, only if it was built for that board")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Lists the USB serial ports, which may have boards on them, with what the computer says of each, without
    /// touching them, including the ID of a board on its chip's own USB, and what each board probably is, from when
    /// it was last identified.
    List,
    /// Lists the boards the platform supports.
    Boards,
    /// The firmware on this computer, and getting more from GitHub releases.
    Firmware {
        #[command(subcommand)]
        command: FirmwareCommands,
    },
    /// Restarts the board, e.g. to see which one it is, by its lights or screen.
    Restart {
        /// The board's serial port, by default the only one plugged in.
        #[arg(short, long, env = "ESPFLASH_PORT")]
        port: Option<String>,
    },
    /// Says which chip the board is, its ID, and which board its firmware was built for. This restarts it.
    Identify {
        /// The board's serial port, by default the only one plugged in.
        #[arg(short, long, env = "ESPFLASH_PORT")]
        port: Option<String>,
    },
    /// Flashes the firmware, if the board's firmware was built for the same board; if it has no record, after asking
    /// which board it is.
    Flash {
        /// The board's serial port, by default the only one plugged in.
        #[arg(short, long, env = "ESPFLASH_PORT")]
        port: Option<String>,
        /// The baud rate, by default 460800 for a board on a USB serial chip, see Port::flash_baud.
        #[arg(short = 'B', long, env = "ESPFLASH_BAUD")]
        baud: Option<u32>,
        /// Shows the board's log after flashing it, until Ctrl+C.
        #[arg(short = 'M', long)]
        monitor: bool,
        /// The board it is, which must be the one the firmware was built for, to flash a board whose firmware has
        /// no record without being asked, or one whose firmware was built for another board.
        #[arg(long)]
        confirm_board: Option<String>,
        /// Agrees to replace the board's firmware with other firmware, e.g. a demo over a capture device's, without
        /// being asked.
        #[arg(long)]
        replace_firmware: bool,
        /// Once flashed, copies the ELF file into this repository's dev version in the shared firmware folder, e.g.
        /// bruce-smart-home, as the dev build scripts do, so that it holds what was last flashed, see
        /// bruce-embedded-firmware. The board crates' cargo runners pass it.
        #[arg(long)]
        keep_as: Option<String>,
        /// The firmware's ELF file, as cargo builds it.
        elf: PathBuf,
    },
}

/// The firmware commands, see bruce-embedded-firmware.
#[derive(Subcommand)]
enum FirmwareCommands {
    /// Lists the firmware on this computer, in the shared firmware folder, by repository and version.
    Local,
    /// Lists the repositories on GitHub with firmware: those with the topic, and those added.
    Find,
    /// Adds a repository, by owner/name, e.g. one without the topic.
    Add { repo: String },
    /// Lists a repository's releases, by owner/name, with their firmware files.
    Releases { repo: String },
    /// Downloads a release of a repository, by owner/name, by default its newest.
    Download { repo: String, version: Option<String> },
    /// Keeps a repository, by owner/name, up to date, its newest release downloaded by update, or with --off, not.
    Keep {
        repo: String,
        #[arg(long)]
        off: bool,
    },
    /// Downloads the newest release of each repository kept up to date.
    Update,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::List => {
            let ports = ports()?;
            if ports.is_empty() {
                println!("No USB serial ports");
            }
            let seen = bruce_embedded_flash::seen::load();
            for port in ports {
                match port.id() {
                    Some(id) => println!("{}: {}, ID {id}", port.name, port.describe()),
                    None => println!("{}: {}", port.name, port.describe()),
                }
                if let Some(guess) = bruce_embedded_flash::seen::guess(&port, &seen) {
                    println!("  {}", guess.describe());
                }
            }
        }
        Commands::Boards => {
            for board in BOARD_TYPES {
                println!("{}: {}, an {}", board.id, board.name, board.chip);
                println!("  {}", board.looks);
                if let Some(url) = board.url {
                    println!("  {url}");
                }
            }
        }
        Commands::Firmware { command } => firmware(command)?,
        Commands::Restart { port } => {
            Connection::open(&port.map_or_else(only_port, Ok)?, None)?.reset()?;
        }
        Commands::Identify { port } => {
            let mut connection = Connection::open(&port.map_or_else(only_port, Ok)?, None)?;
            let board = connection.identify()?;
            connection.reset()?;
            describe(&board);
        }
        Commands::Flash { port, baud, monitor: show_log, confirm_board, replace_firmware, keep_as, elf } => {
            let bytes = std::fs::read(&elf).with_context(|| format!("Unable to read {}", elf.display()))?;
            let image = image_record(&bytes)?;
            let image_type = board_type(image.board());
            println!("The firmware: {} {}, built for {}", image.firmware(), image.version(),
                a(image_type.map_or(image.board(), |board| board.name)));

            let port = port.map_or_else(only_port, Ok)?;
            let baud = baud.or_else(|| ports().ok()?.into_iter().find(|found| found.name == port)?.flash_baud());
            let board = Connection::open(&port, baud)?.identify()?;
            describe(&board);
            board.check_chip(&image)?;

            if let Some(confirmed) = &confirm_board
                && confirmed != image.board()
            {
                bail!("--confirm-board says the board is a {confirmed}, but the firmware was built for a {}",
                    image.board());
            }
            let mut confirmed = confirm_board.clone();
            match board.verdict(&image) {
                Verdict::Same => {}
                Verdict::Different(other) => {
                    if confirm_board.is_none() {
                        bail!("Not flashing: the board's firmware was built for {}, and this for {}. If the \
                            board really is {}, say so with --confirm-board {}",
                            a(board_type(&other).map_or(other.as_str(), |board| board.name)),
                            a(image_type.map_or(image.board(), |board| board.name)), a(image.board()), image.board());
                    }
                    println!("Flashing a board whose firmware was built for {}, as confirmed", a(&other));
                }
                Verdict::Unknown => {
                    if confirm_board.is_none() {
                        // The board runs its firmware while the user looks, and can be restarted to tell which it is.
                        ask(&board, image.board(), image_type, baud)?;
                        confirmed = Some(image.board().to_string());
                    }
                }
            }
            if let Some(replaced) = board.replaced_firmware(&image)
                && !replace_firmware
            {
                ask_replace(&replaced, image.firmware())?;
            }
            let confirmed = Confirmed { board: confirmed, replace_firmware: true };
            flash_board(&port, baud, &bytes, &board.id, &confirmed, &mut Progress::default())?;
            println!("Flashed, and restarted the board");
            if let Some(repo) = keep_as {
                let kept = bruce_embedded_firmware::keep_dev(&repo, &elf, image.firmware(), image.board())?;
                println!("Kept it as {}", kept.display());
            }
            if show_log {
                monitor(&port, &mut std::io::stdout())?;
            }
        }
    }
    Ok(())
}

/// Runs a firmware command.
fn firmware(command: FirmwareCommands) -> anyhow::Result<()> {
    use bruce_embedded_firmware::github;
    match command {
        FirmwareCommands::Local => {
            println!("In {}", bruce_embedded_firmware::folder()?.display());
            for local in bruce_embedded_firmware::local()? {
                let what = std::fs::read(&local.path)
                    .map_err(anyhow::Error::from)
                    .and_then(|elf| image_record(&elf))
                    .map(|record| format!("{} {}, for {}", record.firmware(), record.version(), a(&board_name(record.board()))))
                    .unwrap_or_else(|error| format!("unusable, {error:#}"));
                println!("  {}: {what}", local.label());
            }
        }
        FirmwareCommands::Find => {
            let (repos, errors) = github::find()?;
            let settings = github::Settings::load()?;
            for repo in repos {
                let kept = if settings.is_kept_up_to_date(&repo.full_name()) { ", kept up to date" } else { "" };
                println!("{}: {}{kept}", repo.full_name(), repo.short);
            }
            for error in errors {
                println!("Left out {error}");
            }
        }
        FirmwareCommands::Add { repo } => {
            let found = github::repo(&repo)?;
            let mut settings = github::Settings::load()?;
            if !settings.added.contains(&repo) {
                settings.added.push(repo);
                settings.save()?;
            }
            println!("Added {}: {}", found.full_name(), found.short);
        }
        FirmwareCommands::Releases { repo } => {
            let repo = github::repo(&repo)?;
            for release in github::releases(&repo)? {
                let downloaded = if github::is_downloaded(&repo, &release)? { ", downloaded" } else { "" };
                let pre = if release.prerelease { ", a prerelease" } else { "" };
                println!("{} ({}), {}{pre}{downloaded}", release.version, release.name, release.published);
                for asset in &release.assets {
                    println!("  {}, {} KB", asset.name, asset.size / 1024);
                }
            }
        }
        FirmwareCommands::Download { repo, version } => {
            let repo = github::repo(&repo)?;
            let releases = github::releases(&repo)?;
            let release = match &version {
                Some(version) => releases.iter().find(|release| &release.version == version),
                None => github::newest(&releases),
            }
            .ok_or_else(|| anyhow::anyhow!("{} has no such release with firmware", repo.full_name()))?;
            let downloaded = github::download(&repo, release)?;
            println!("Downloaded {} files of {} {} to {}", downloaded.len(), repo.full_name(), release.version,
                github::release_folder(&repo, release)?.display());
        }
        FirmwareCommands::Keep { repo, off } => {
            let mut settings = github::Settings::load()?;
            settings.set_kept_up_to_date(&repo, !off);
            settings.save()?;
        }
        FirmwareCommands::Update => {
            let (downloaded, errors) = github::update()?;
            for path in downloaded {
                println!("Downloaded {}", path.display());
            }
            for error in errors {
                println!("Unable to update {error}");
            }
        }
    }
    Ok(())
}

/// The board's full name, or its id if it is not one this knows.
fn board_name(id: &str) -> String {
    board_type(id).map_or(id.to_string(), |board| board.name.to_string())
}

/// Prints what the board is and which board its firmware was built for.
fn describe(board: &Board) {
    println!("The board on {}: an {}{}, ID {}, {} of flash", board.port, board.chip,
        board.revision.as_ref().map(|revision| format!(" {revision}")).unwrap_or_default(), board.id,
        board.flash_size);
    if board.apps.is_empty() {
        println!("  It has no partition table, so no firmware");
    }
    for app in &board.apps {
        match &app.record {
            Some(record) => println!("  {} at 0x{:x}: {} {}, built for {}", app.label, app.offset,
                record.firmware(), record.version(), a(board_type(record.board()).map_or(record.board(), |b| b.name))),
            None => println!("  {} at 0x{:x}: firmware with no record of its board", app.label, app.offset),
        }
    }
}

/// Asks the user to say that the board, whose firmware has no record, is the one the firmware was built for, by
/// typing its id, which is harder to do without looking than answering yes, offering to restart it, as many times as
/// they like, so that they can tell which board it is, by its lights or screen, when several are plugged in.
fn ask(board: &Board, image_board: &str, image_type: Option<&BoardType>, baud: Option<u32>) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!("Not flashing: the board's firmware does not say which board it is. If it really is a {image_board}, \
            say so with --confirm-board {image_board}");
    }
    println!();
    println!("The board's firmware does not say which board it is, e.g. as it is new, or was last flashed before");
    println!("firmware records. Firmware built for another board can damage it.");
    if let Some(board_type) = image_type {
        println!();
        println!("Is the board on {}, ID {}, {}?", board.port, board.id, a(board_type.name));
        println!("  {}", board_type.looks);
        if let Some(url) = board_type.url {
            println!("  {url}");
        }
    }
    loop {
        print!("If it is, type {image_board}; type r to restart it, to see which board it is; anything else stops: ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        match answer.trim() {
            "r" => {
                Connection::open(&board.port, baud)?.reset()?;
                println!("Restarted the board on {}", board.port);
            }
            answer if answer == image_board => return Ok(()),
            _ => bail!("Not flashing"),
        }
    }
}

/// Asks the user to agree to replace the board's firmware with other firmware, as it would stop doing what it did.
fn ask_replace(replaced: &str, firmware: &str) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!("Not flashing: the board runs {replaced}, which {firmware} would replace. To replace it, say so with \
            --replace-firmware");
    }
    println!();
    println!("The board runs {replaced}, which {firmware} would replace, so it would stop doing what it does.");
    print!("To replace it, type replace: ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    if answer.trim() != "replace" {
        bail!("Not flashing");
    }
    Ok(())
}

/// Shows how far flashing has got, a line per region of flash, overwritten as it goes.
#[derive(Default)]
struct Progress {
    address: u32,
    total: usize,
}

impl ProgressCallbacks for Progress {
    fn init(&mut self, address: u32, total: usize) {
        self.address = address;
        self.total = total.max(1);
        print!("\r0x{address:06x}: 0%");
        let _ = std::io::stdout().flush();
    }

    fn update(&mut self, current: usize) {
        print!("\r0x{:06x}: {}%", self.address, current * 100 / self.total);
        let _ = std::io::stdout().flush();
    }

    fn verifying(&mut self) {
        print!("\r0x{:06x}: verifying", self.address);
        let _ = std::io::stdout().flush();
    }

    fn finish(&mut self, skipped: bool) {
        println!("\r0x{:06x}: {}", self.address, if skipped { "already there, skipped" } else { "done      " });
    }
}
