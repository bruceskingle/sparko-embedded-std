//! Where the RAM is, for GET /api/memory, see api.rs: the heap, by the kind of memory, and how close each task has come
//! to the end of its stack.

use serde_json::{Value, json};

/// The heap, by the kind of memory, each with its size, how much is free, the least that has been since the device
/// started, and the largest block which could be allocated, and the tasks, see tasks.
pub fn to_json() -> Value {
    let heap = |caps: u32| {
        let mut info = esp_idf_sys::multi_heap_info_t::default();
        unsafe { esp_idf_sys::heap_caps_get_info(&mut info, caps) };
        json!({
            "size": unsafe { esp_idf_sys::heap_caps_get_total_size(caps) },
            "free": info.total_free_bytes,
            "minimumFree": info.minimum_free_bytes,
            "largestFreeBlock": info.largest_free_block,
            "allocated": info.total_allocated_bytes,
            "allocatedBlocks": info.allocated_blocks,
        })
    };
    json!({
        "heap": {
            // What malloc uses, so what Rust allocates from.
            "default": heap(esp_idf_sys::MALLOC_CAP_DEFAULT),
            "internal": heap(esp_idf_sys::MALLOC_CAP_INTERNAL | esp_idf_sys::MALLOC_CAP_8BIT),
            "dma": heap(esp_idf_sys::MALLOC_CAP_DMA),
            "psram": heap(esp_idf_sys::MALLOC_CAP_SPIRAM),
        },
        "tasks": tasks(),
    })
}

/// Each task's name, core, priority and the least stack it has had free, in bytes, as ESP-IDF's stacks are counted in
/// bytes, which needs CONFIG_FREERTOS_USE_TRACE_FACILITY, otherwise null.
#[cfg(esp_idf_freertos_use_trace_facility)]
fn tasks() -> Value {
    let count = unsafe { esp_idf_sys::uxTaskGetNumberOfTasks() } as usize;
    // Room for a few more, in case tasks start while this runs.
    let mut statuses = vec![esp_idf_sys::TaskStatus_t::default(); count + 4];
    let count = unsafe {
        esp_idf_sys::uxTaskGetSystemState(statuses.as_mut_ptr(), statuses.len() as _, std::ptr::null_mut())
    } as usize;
    let mut tasks: Vec<Value> = statuses[..count]
        .iter()
        .map(|status| {
            let name = unsafe { std::ffi::CStr::from_ptr(status.pcTaskName) }.to_string_lossy();
            json!({
                "name": name,
                "priority": status.uxCurrentPriority,
                "stackFreeMinimum": status.usStackHighWaterMark,
            })
        })
        .collect();
    tasks.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Value::Array(tasks)
}

#[cfg(not(esp_idf_freertos_use_trace_facility))]
fn tasks() -> Value {
    Value::Null
}
