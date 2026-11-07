# bruce-embedded-flash

Flashes firmware built on the platform to a board only if it was built for that board. Firmware built for the wrong
board drives the wrong pins, and once broke a board's display.

## How it knows

Each firmware image carries a record of the board it was built for, the firmware and its version
([bruce-embedded-firmware-record](../../libs/bruce-embedded-firmware-record)), which a board crate adds with
`.with_firmware(sparko_esp_idf::firmware!())`. The linker puts it straight after ESP-IDF's app descriptor, at 0x120 in
the image, starting with the magic `bruce-embedded-std firmware 1`. The firmware also logs it as it starts, with the
device's ID, e.g.

    bruce-embedded-std firmware 1: board=guition-jc3248w535c firmware=marco-sparko-capture version=0.1.0 id=a0b1c2d3e4f5

Before flashing, the tool reads the record from the ELF file, and from each app partition on the board, over the serial
port from its ROM bootloader, so it works whether or not the firmware on the board runs. Then:

- if the board's firmware was built for the same board, it flashes;
- if it has no record, e.g. a new board, or one last flashed before records, it asks you whether it is the board the
  firmware is for, by its full name, what it looks like, and its maker's page, from the catalogue of supported boards
  (`BOARD_TYPES`), and to type its id, or `--confirm-board`; it offers to restart the board, as often as you like,
  so that you can tell which board it is by its lights or screen when several are plugged in;
- if it was built for another board, it refuses, unless `--confirm-board` names the board the new firmware is for;
- if the firmware is for another chip, it refuses, whatever you say;
- if the board runs other firmware, e.g. a capture device's, which the new firmware, e.g. a board's demo, would
  replace, it asks you to agree to replace it, by typing `replace`, or `--replace-firmware`. An app which knows its
  devices, e.g. Marco Sparko, asks too before flashing anything but its own firmware to one of them, even if the
  firmware on it does not say what it is.

## Using it

Install it with `cargo install --path tools/bruce-embedded-flash`. It flashes in its own code, with espflash's library,
so it needs no other tools.

- `bruce-embedded-flash list` lists the USB serial ports, without touching the boards on them, with the ID of each
  board on its chip's own USB (an ESP32-S3's or -C6's), which is its USB serial number, so that a board can be found
  without restarting the others. A board with a USB serial chip, e.g. a CYD, does not say.
  Each board identified is remembered, in `boards-seen.json` beside the shared firmware folder, so the list says
  what each port's board was when last identified: for sure by ID, or only maybe, by its port, for a board on a USB
  serial chip, as macOS names such a port for its USB socket, or its chip's serial number, which many chips share.
- `bruce-embedded-flash boards` lists the supported boards.
- `bruce-embedded-flash restart [--port PORT]` restarts a board, e.g. to see which one it is.
- `bruce-embedded-flash identify [--port PORT]` says the board's chip, ID and which board its firmware was built for.
- `bruce-embedded-flash flash [--port PORT] [--baud N] [--monitor] [--confirm-board BOARD] [--replace-firmware] ELF`
  flashes.

- `bruce-embedded-flash firmware local|find|add|releases|download|keep|update` lists the firmware on this computer,
  in the shared firmware folder (`<data dir>/bruce-embedded-std/firmware/<repo>/<version>/`), finds the repositories
  on GitHub with the topic `bruce-embedded-firmware` and a `bruce-embedded-std.md`, whose first line, a heading, is
  their short description and the rest their long one, lists and downloads their releases' ELF files, and keeps
  chosen ones up to date, see `tools/bruce-embedded-firmware`.

`flash --keep-as <repo>` copies the ELF file, once flashed, into that repository's `dev` version in the shared
firmware folder, named as the dev build scripts name it, so that it holds what was last flashed. The board crates'
cargo runners pass `--keep-as bruce-embedded-std`, and marco-sparko's capture firmware's `--keep-as bruce-smart-home`,
so `cargo run --release` keeps what it flashes; `./build.sh` and `capture/build-firmware.sh` keep every board's.

The port defaults to `ESPFLASH_PORT`, or the only board plugged in. All but `list` and `boards` restart the board. It
is the cargo runner of the board crates and of marco-sparko's capture firmware, so `cargo run --release` checks too.

## In an app

The library (`ports`, `Connection::open`, `identify`, `flash`, `reset`, `Board::verdict`, `Board::check_chip`,
`BOARD_TYPES`) is what an app uses to flash boards on its users' computers. It uses `serialport`, which is MPL-2.0,
unmodified. It needs Rust 1.94, so cargo picks espflash 4.5, as 4.7 needs 1.95.

The Dioxus component `Flasher`, in [bruce-embedded-flash-ui](../bruce-embedded-flash-ui), shows the same flow in a
page: the boards on USB, a board plugged in identified at once, the boards it could be by full name, the confirmation,
with Restart to see which board it is, and progress. Marco Sparko's Devices page uses it, and so does the desktop app,
`cargo run --features desktop -- <ELF>...` in that folder.

