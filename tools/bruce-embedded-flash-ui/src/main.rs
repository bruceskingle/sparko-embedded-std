//! bruce-embedded-flash-gui: a window listing the boards on USB, which flashes the firmware in the shared firmware
//! folder, see bruce-embedded-firmware, and in the ELF files named on its command line to one of them, only if it was
//! built for that board, as the user confirms, see the Flasher component.

use std::path::PathBuf;

use bruce_embedded_flash_ui::{Firmware, Flasher, GetFirmware};
use dioxus::prelude::*;

/// The window's look, beside the component's own.
const STYLE: &str = r#"
:root { color-scheme: light dark; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 1em 1.5em; }
.firmware-error { color: #d03030; }
"#;

fn main() {
    dioxus::LaunchBuilder::new()
        .with_cfg(dioxus::desktop::Config::new().with_window(
            dioxus::desktop::WindowBuilder::new().with_title("Bruce's Embedded Flasher"),
        ))
        .launch(App);
}

/// The firmware in the shared firmware folder and named on the command line.
fn load() -> Vec<Result<Firmware, String>> {
    let mut loaded = Firmware::load_local();
    loaded.extend(std::env::args_os().skip(1).map(PathBuf::from).map(|path| Firmware::load(&path)));
    loaded
}

#[component]
fn App() -> Element {
    // The firmware in the shared firmware folder and named on the command line, and why any could not be read, read
    // again when more is downloaded.
    let mut reload = use_signal(load);
    let mut getting = use_signal(|| false);
    let loaded = reload.read().clone();
    let firmware: Vec<Firmware> = loaded.iter().filter_map(|loaded| loaded.clone().ok()).collect();

    rsx! {
        style { {STYLE} }
        h1 { "Flash a Board" }
        if firmware.is_empty() {
            p { "No firmware: download some, or name the firmware's ELF files on the command line." }
        } else {
            p { "Firmware:" }
            ul {
                for image in firmware.iter() {
                    li { "{image.record.firmware()} {image.record.version()}, for a {image.record.board()}, from {image.label}" }
                }
            }
        }
        for error in loaded.iter().filter_map(|loaded| loaded.clone().err()) {
            p { class: "firmware-error", "{error}" }
        }
        p {
            if getting() {
                button { onclick: move |_| getting.set(false), "Hide Get Firmware" }
            } else {
                button { onclick: move |_| getting.set(true), "Get Firmware" }
            }
        }
        if getting() {
            h2 { "Get Firmware" }
            GetFirmware { on_downloaded: move |_| reload.set(load()) }
        }
        Flasher { firmware, known: Vec::new(), preferred: None, baud: None }
    }
}
