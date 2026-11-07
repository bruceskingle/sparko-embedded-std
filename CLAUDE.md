# sparko-embedded-std

A Rust `std` platform for ESP32 boards (on `esp-idf-svc`/`esp-idf-hal`), built as a plugin framework: a board's
`main.rs` builds an `Esp32Platform` and adds the *Features* it wants. See README.md for the concepts; parts of it
(the `examples` directory, links to `sparko-esp-idf` and `sparko-embedded-example`) predate the mono repo. Those repos
are retired: everything is here.

## Layout

- `libs/sparko-embedded-std` - hardware independent core: config (`TypedValue`, `ConfigSpec`, `FeatureConfig`),
  `ConfigManager`, the cron `TaskScheduler`, the `HttpServerManager` trait, `DisplayManager`, graphics (clock),
  time zones, `Listener`.
- `libs/sparko-esp-idf` - the ESP32 implementation: `Esp32Platform` (builder, `Remainder` of unused peripherals per
  board, runner), the `Feature`/`DynFeature` traits, the Core feature (WiFi, AP mode captive portal, HTTP server,
  mDNS, SNTP), config in NVS, LEDs, smart LEDs (RMT or SPI), MIPI DSI displays, touch, IMU/AHRS, and the features in
  `src/features` (AnalogClock, BinaryClock, DynDns2).
- `libs/feature-config-derive` - `#[derive(FeatureConfig)]` for feature config structs (`#[config(len = N)]` for
  strings).
- `boards/<board>` - one binary crate per board, each its own Cargo project (not a workspace member), with its own
  `.cargo/config.toml` (target, `MCU`, ESP-IDF version), `rust-toolchain.toml` (`esp`), `sdkconfig.defaults` and
  `Cargo.lock`.

## Boards

| Board | Cargo feature | Target | MCU |
|---|---|---|---|
| cyd (ESP32-2432S028R, 2.8" 320x240 display, no PSRAM) | `board-cyd-esp32-28` | `xtensa-esp32-espidf` | esp32 |
| esp32-devkit-v1 | `board-esp32-devkit-v1` | `xtensa-esp32-espidf` | esp32 |
| esp32c3-supermini | `board-esp32c3-supermini` | `riscv32imc-esp-espidf` | esp32c3 |
| wave-esp32c6-lcd-147 | `board-wave-esp32c6-lcd-147` | `riscv32imac-esp-espidf` | esp32c6 |
| wave-esp32c6-touch-147 | `board-wave-esp32c6-touch-147` (+ `tilt`, `ahrs`) | `riscv32imac-esp-espidf` | esp32c6 |
| seeed-esp32c6-xiao | `board-seeed-esp32c6-xiao` | `riscv32imac-esp-espidf` | esp32c6 |
| wave-esp32s3-rlcd-42 (4.2" reflective LCD, 8 MB PSRAM) | `board-wave-esp32s3-rlcd-42` | `xtensa-esp32s3-espidf` | esp32s3 |

Board names are the maker, the chip, then the maker's model with its size without the dot, e.g. Waveshare's
ESP32-S3-RLCD-4.2 is `wave-esp32s3-rlcd-42`; an unbranded board starts with its chip, e.g. `esp32c3-supermini`.
Capture firmware features in marco-sparko use the same names.

Each board crate selects exactly one `board-*` feature of `sparko-esp-idf`, which turns on the functional features
(`display`, `rgb-led`, `smart-led-spi`, ...) its hardware has. Code behind a functional feature only compiles for
boards which have it.

### Board features, optional hardware and the Remainder

This is the model every piece of on-board hardware follows, so that the platform is easy to use and hard to get wrong:

- **Board features.** The project holding `main` sets exactly one `board-*` feature, which turns on the capabilities
  that board always has, e.g. `rgb-led` or `display`. The platform drives those itself.
- **Optional hardware features.** Hardware a project may or may not want, because its pins could be used for something
  else, e.g. the SD card slot, has its own feature (`sd-card`), which the project sets to enable it.
  - Setting it for a board which does not have that hardware is a **compile error**, with a message saying so, not an
    error at run time, nor an accidental compile error elsewhere.
  - Setting it **consumes** the hardware's pins and peripherals, e.g. GPIO N and its SPI bus, which are then **not in
    the `Remainder`**, so a project cannot use them by mistake.
  - Not setting it leaves every one of those pins and peripherals **in the `Remainder`** for the project to use for
    other things. None is taken or dropped just because the board has the slot.
- **Only header pins are in the `Remainder`.** A pin which is not on one of the board's headers can never be used for
  anything else, so it never appears in the `Remainder`. On-board hardware all of whose pins are off the headers is
  then simply part of the board: its `board-*` feature enables it by default, rather than it being an optional feature.
  E.g. the CYD's SD slot (GPIO 5, 18, 19, 23) is wired only to the slot, so `board-cyd-esp32-28` should enable `sd-card`.
- Pins which are only ever used by always-on hardware, e.g. the display's, are never in the `Remainder`. A pin shared
  between always-on and optional hardware, e.g. an SPI bus the display and the SD slot share, is the platform's; only
  the pins the optional hardware alone uses move between it and the `Remainder`.

## Building and flashing

- Build a board from its own directory: `cd boards/cyd-esp32-28 && cargo build --release`. Always `--release`. Never build the
  libs on their own or several boards as one workspace: the target and features come from the board.
- `./build.sh` builds every board in turn (slow), or those named, e.g. `./build.sh seeed-esp32c6-xiao`, and copies each
  demo to the shared firmware folder as `bruce-embedded-std/dev/`, see `tools/bruce-embedded-firmware`.
- Flash and monitor: `cargo run --release` in the board directory, which also keeps the ELF in the shared firmware
  folder's `bruce-embedded-std/dev/`. The runner is `bruce-embedded-flash flash --monitor --keep-as bruce-embedded-std`
  (`tools/bruce-embedded-flash`, install it with `cargo install --path tools/bruce-embedded-flash`), which flashes only
  firmware built for the board on the port, from the firmware record each image carries
  (`libs/bruce-embedded-firmware-record`, added by `.with_firmware(sparko_esp_idf::firmware!())`), see its README. Never
  flash with espflash directly, and never pass `--confirm-board` on the owner's behalf. Ask before flashing: it needs
  the board plugged in and replaces what is on it.
- `tools/` holds host programs, each its own Cargo project built with the default toolchain, not `esp`:
  `bruce-embedded-firmware`, the shared firmware folder, `<data dir>/bruce-embedded-std/firmware/<repo>/<version>/`,
  e.g. `bruce-smart-home/dev/` for marco-sparko's capture builds, and later the downloader from GitHub releases,
  `bruce-embedded-flash`, the flashing library and command line, and `bruce-embedded-flash-ui`, the Dioxus component,
  `Flasher`, which marco-sparko's Devices page uses, and the desktop app built with it,
  `cargo run --features desktop -- <ELF>...`.
- ESP-IDF tools are installed globally (`ESP_IDF_TOOLS_INSTALL_DIR = "global"`, in `~/.espressif`), not per project.
- The cyd-esp32-28 board depends on a local fork at `../../../ws2812-esp32-rmt-driver` (i.e. `~/git/ws2812-esp32-rmt-driver`).

## rust-analyzer

The VS Code workspace `sparko-platform.code-workspace` lists one board in `rust-analyzer.linkedProjects` at a time
(plus `libs/sparko-embedded-std`), so that only the platform code that board enables is analysed and the other boards'
hardware does not show as errors. Switch the linked board to work on another. Don't link `libs/sparko-esp-idf` on its
own (it gives odd errors).

## Writing a feature

Implement `sparko_esp_idf::Feature`: `init` returns a `FeatureDescriptor` (name and `Config::to_config_spec()`), and
`start` gets the platform, the initializer and the typed config. Do periodic work by adding a `ScheduledTask` with a
cron spec (seconds field first, e.g. `"0 */5 * * * *"`). Features can live outside the libs, e.g. in a board's
`main.rs` (see wave-esp32c6-touch-147). The scheduler runs every task in turn on one thread, so a slow task (e.g. an
HTTPS request) delays the others, e.g. the clock.

Config values are stored in NVS and edited on the device's `/config` page, which has no login. Mark passwords and keys
`#[config(secret)]` (string fields only): the page never sends them to the browser (an empty field keeps the saved
value, optional ones get a Clear Password button which only takes effect on Save) and `ConfigSpecValue`'s Debug and the config store's logs redact them. Don't
log a secret's value anywhere else either.

## HTTP server

`HttpServerManager` only supports fixed status pages whose body a closure writes. Anything needing the request (query
string, status codes) uses `EspHttpServerManager::on` directly. Making the HTTP server hardware independent has been
tried twice and does not work with the ESP HTTP server and Rust's type system: do not try again (see `http.rs`).

## Device API

When a board's `sdkconfig.defaults` sets `CONFIG_ESP_HTTPS_SERVER_ENABLE=y` (with the mbedtls dynamic buffer options,
see marco-sparko's `capture/sdkconfig.defaults`), the platform runs an HTTPS API on port 443 once on the Wi-Fi
(`api.rs`, cfg `esp_idf_esp_https_server_enable`, made visible by `sparko-esp-idf/build.rs`), next to the HTTP server,
which stays for the captive portal and `/config`. The device makes an EC P-256 key and self-signed certificate on
first boot and keeps them in the `device` NVS namespace (`identity.rs`); its ID is the Wi-Fi MAC. Endpoints:
`GET /api/info`, `GET`/`POST /api/config` (JSON from `ConfigSpec`, `config_json.rs` in sparko-embedded-std; secrets
only as `isSet`; POST changes only what it names, checks everything before saving, null clears an optional setting,
takes effect after a restart), `GET /api/memory` (heap by kind, task stack high-water marks with
`CONFIG_FREERTOS_USE_TRACE_FACILITY`), `GET /api/starts` (each start's time and reset reason, the latest 30 logged in NVS, the device's own, before the
features start, and counted in NVS from the first moment, `starts.rs`), `POST /api/restart`. `/api/info` includes
`upTimeSeconds` and `resetReason`.
Features add their own with `sparko.api_server`'s `on`, or for reading, `sparko.api_get`, which also serves them over
plain HTTP on port 80, as does `/api/info`. Board crates name their firmware with `with_firmware`.

Adoption (`adoption.rs`): a pairing code on the screen, then the app's key in NVS; after that every request but
`/api/info` needs `Authorization: Bearer <key>` over HTTPS, or, for `api_get` reads over HTTP,
`Sparko-HMAC-SHA256 <unix time>:<hex HMAC-SHA256 of "<method> <uri> <time>" with the key's 32 bytes>` within 60 s
of the device's clock, 5 min, as the HTTP server serves one request at a time (`Adoption::is_signed`), so the key never crosses the network in clear. HTTPS connections cost
tens of KB of heap each on the CYD (no PSRAM), which is why reads use HTTP; changes and adoption stay on HTTPS.

## Conventions

- Builders: `Type::builder()`, chainable `with_*`, callable `add_*`, `build()` shrinks collections, which are then
  treated as immutable.
- Fallible functions return `anyhow::Result`.
- Log with the `log` macros (`info!`, `warn!`, ...), never `println!`, `print!` or `eprintln!`. On a chip whose
  console is its own USB port, e.g. the ESP32-C6, writing to stdout fails when no computer is reading the port, and
  `println!` panics on a failed write ("failed printing to stdout"), so the device restarts on a charger or a hub but
  not on a computer. The `log` macros go through ESP-IDF's logging, which drops what it cannot send.

## Working with the owner

- Never commit, push or offer to: the owner reviews and commits.
- This repo is developed alongside `../marco-sparko`, a desktop app which reads data from ESP32 "Capture Devices"
  built on this platform, whose firmware is in marco-sparko's `capture/`. Claude's memory of each repo is kept with it,
  and marco-sparko's, the workspace's, lists this one's too.
- The device pages' look, `libs/sparko-embedded-std/src/http_server.rs`, is just what could be written by hand; the
  owner would prefer a dark theme, when they are redesigned, which is not planned yet.
