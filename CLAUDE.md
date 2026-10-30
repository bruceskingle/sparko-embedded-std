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
| cyd (ESP32-2432S028R, 2.8" 320x240 display, no PSRAM) | `board-cyd` | `xtensa-esp32-espidf` | esp32 |
| devkitv1 | `board-devkitv1` | `xtensa-esp32-espidf` | esp32 |
| supermini-esp32c3 | `board-supermini-esp32c3` | `riscv32imc-esp-espidf` | esp32c3 |
| wave-esp32c6147 | `board-wave-esp32c6147` | `riscv32imac-esp-espidf` | esp32c6 |
| wave-esp32c6touch147 | `board-wave-esp32c6touch147` (+ `tilt`, `ahrs`) | `riscv32imac-esp-espidf` | esp32c6 |
| xiao-esp32c6 | `board-xiao-esp32c6` | `riscv32imac-esp-espidf` | esp32c6 |

Each board crate selects exactly one `board-*` feature of `sparko-esp-idf`, which turns on the functional features
(`display`, `rgb-led`, `smart-led-spi`, ...) its hardware has. Code behind a functional feature only compiles for
boards which have it.

## Building and flashing

- Build a board from its own directory: `cd boards/cyd && cargo build --release`. Always `--release`. Never build the
  libs on their own or several boards as one workspace: the target and features come from the board.
- `./build.sh` builds every board in turn (slow).
- Flash and monitor: `cargo run --release` in the board directory (the runner is `espflash flash --monitor`). Ask
  before flashing: it needs the board plugged in and replaces what is on it.
- ESP-IDF tools are installed globally (`ESP_IDF_TOOLS_INSTALL_DIR = "global"`, in `~/.espressif`), not per project.
- The cyd board depends on a local fork at `../../../ws2812-esp32-rmt-driver` (i.e. `~/git/ws2812-esp32-rmt-driver`).

## rust-analyzer

The VS Code workspace `sparko-platform.code-workspace` lists one board in `rust-analyzer.linkedProjects` at a time
(plus `libs/sparko-embedded-std`), so that only the platform code that board enables is analysed and the other boards'
hardware does not show as errors. Switch the linked board to work on another. Don't link `libs/sparko-esp-idf` on its
own (it gives odd errors).

## Writing a feature

Implement `sparko_esp_idf::Feature`: `init` returns a `FeatureDescriptor` (name and `Config::to_config_spec()`), and
`start` gets the platform, the initializer and the typed config. Do periodic work by adding a `ScheduledTask` with a
cron spec (seconds field first, e.g. `"0 */5 * * * *"`). Features can live outside the libs, e.g. in a board's
`main.rs` (see wave-esp32c6touch147). The scheduler runs every task in turn on one thread, so a slow task (e.g. an
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
`CONFIG_FREERTOS_USE_TRACE_FACILITY`), `GET /api/starts` (each start's time and reset reason, logged on the SD card in `device/starts.jsonl` before the
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

## Working with the owner

- Never commit, push or offer to: the owner reviews and commits.
- This repo is developed alongside `../marco-sparko`, a desktop app which will read data from ESP32 "Capture Devices"
  built on this platform; Claude's project memory for both lives with marco-sparko.
