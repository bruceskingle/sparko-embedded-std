//! Why the device last crashed, kept over the restart it causes in memory which a restart does not clear, so that why a
//! device which is not plugged into a computer, and so has no log to read, restarted can be seen: on the screen while
//! it joins the Wi-Fi, see Esp32Platform::start_client, and in /api/info, see api.rs.
//!
//! That is the message of the last Rust panic, or if there was none, the last allocation which failed, as running out
//! of memory aborts without a panic, so the panic hook never sees it. ESP-IDF calls on_failed_allocation for every
//! allocation which fails, Rust's or C's, some of which are handled, e.g. by mbedtls, so a failed allocation is only
//! given as the reason for a restart whose reset reason is Panic.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// Marks the message as one a panic wrote, rather than what the memory held at power on.
const MAGIC: u32 = 0x5041_4e43;
/// Marks the failed allocation as one on_failed_allocation wrote.
const ALLOCATION_MAGIC: u32 = 0x414c_4c43;
const LENGTH: usize = 200;

#[repr(C)]
struct Kept {
    magic: u32,
    length: u32,
    message: [u8; LENGTH],
}

/// The last allocation which failed: its size, and the free heap and largest free block then, in bytes.
#[repr(C)]
struct KeptAllocation {
    magic: u32,
    size: u32,
    free: u32,
    largest: u32,
}

#[unsafe(link_section = ".rtc_noinit")]
static mut KEPT: Kept = Kept { magic: 0, length: 0, message: [0; LENGTH] };
#[unsafe(link_section = ".rtc_noinit")]
static mut KEPT_ALLOCATION: KeptAllocation = KeptAllocation { magic: 0, size: 0, free: 0, largest: 0 };

static LAST: OnceLock<Option<String>> = OnceLock::new();
/// How many allocations have failed since the device started, which it may have recovered from.
static FAILED_ALLOCATIONS: AtomicU32 = AtomicU32::new(0);

/// Called by ESP-IDF when an allocation fails, before it returns nothing to the caller, so it must not allocate.
unsafe extern "C" fn on_failed_allocation(size: usize, _caps: u32, _function_name: *const core::ffi::c_char) {
    FAILED_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    let caps = esp_idf_sys::MALLOC_CAP_DEFAULT;
    let kept = unsafe { &mut *std::ptr::addr_of_mut!(KEPT_ALLOCATION) };
    kept.size = size as u32;
    kept.free = unsafe { esp_idf_sys::heap_caps_get_free_size(caps) } as u32;
    kept.largest = unsafe { esp_idf_sys::heap_caps_get_largest_free_block(caps) } as u32;
    kept.magic = ALLOCATION_MAGIC;
}

/// Takes why the device crashed before this start, if it did, and keeps the message of any panic, and the details of
/// any failed allocation, from now on, as well as the default hook's logging the panic.
pub fn install() {
    LAST.get_or_init(|| {
        // Only here, before the hook is installed, and in the hook, which aborts after it, and the callback.
        let kept = unsafe { &mut *std::ptr::addr_of_mut!(KEPT) };
        let allocation = unsafe { &mut *std::ptr::addr_of_mut!(KEPT_ALLOCATION) };
        let panic = (kept.magic == MAGIC).then(|| {
            let length = (kept.length as usize).min(LENGTH);
            String::from_utf8_lossy(&kept.message[..length]).into_owned()
        });
        let out_of_memory = (allocation.magic == ALLOCATION_MAGIC && crate::reset_reason() == "Panic").then(|| format!(
            "Out of memory: {} bytes could not be allocated, with {} free, the largest block {}",
            allocation.size, allocation.free, allocation.largest));
        kept.magic = 0;
        allocation.magic = 0;
        panic.or(out_of_memory)
    });
    if let Some(last) = last() {
        log::error!("Restarted after a crash, {last}");
    }
    let result = unsafe { esp_idf_sys::heap_caps_register_failed_alloc_callback(Some(on_failed_allocation)) };
    if result != esp_idf_sys::ESP_OK {
        log::warn!("Unable to watch for failed allocations, {result}");
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

/// Why the device crashed before this start, if it did: the panic's message, e.g. "panicked at src/wifi.rs:300:5:
/// ...", or that it ran out of memory, see the module comment.
pub fn last() -> Option<&'static str> {
    LAST.get().and_then(|last| last.as_deref())
}

/// How many allocations have failed since the device started.
pub fn failed_allocations() -> u32 {
    FAILED_ALLOCATIONS.load(Ordering::Relaxed)
}
