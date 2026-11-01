//! The message of the last Rust panic, kept over the restart it causes in memory which a restart does not clear, so
//! that why a device which is not plugged into a computer, and so has no log to read, restarted can be seen, see
//! Esp32Platform::start_client, which shows it while the device joins the Wi-Fi.

use std::sync::OnceLock;

/// Marks the message as one a panic wrote, rather than what the memory held at power on.
const MAGIC: u32 = 0x5041_4e43;
const LENGTH: usize = 200;

#[repr(C)]
struct Kept {
    magic: u32,
    length: u32,
    message: [u8; LENGTH],
}

#[unsafe(link_section = ".rtc_noinit")]
static mut KEPT: Kept = Kept { magic: 0, length: 0, message: [0; LENGTH] };

static LAST: OnceLock<Option<String>> = OnceLock::new();

/// Takes the message of the panic before this start, if there was one, and keeps the message of any panic from now
/// on, as well as the default hook's logging it.
pub fn install() {
    LAST.get_or_init(|| {
        // Only here, before the hook is installed, and in the hook, which aborts after it.
        let kept = unsafe { &mut *std::ptr::addr_of_mut!(KEPT) };
        let last = (kept.magic == MAGIC).then(|| {
            let length = (kept.length as usize).min(LENGTH);
            String::from_utf8_lossy(&kept.message[..length]).into_owned()
        });
        kept.magic = 0;
        last
    });
    if let Some(last) = last() {
        log::error!("Restarted after a panic, {last}");
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info.to_string();
        let kept = unsafe { &mut *std::ptr::addr_of_mut!(KEPT) };
        let length = message.len().min(LENGTH);
        kept.message[..length].copy_from_slice(&message.as_bytes()[..length]);
        kept.length = length as u32;
        kept.magic = MAGIC;
        default(info);
    }));
}

/// The message of the panic before this start, e.g. "panicked at src/wifi.rs:300:5: ...", if there was one.
pub fn last() -> Option<&'static str> {
    LAST.get().and_then(|last| last.as_deref())
}
