//! A log of the device's starts, and why each happened, e.g. Panic, or Brownout, see reset_reason, as it only knows
//! why it last started, so that restarts in the night, which only show as gaps in what it captured, can be seen, from
//! GET /api/starts, see to_json.
//!
//! Each start is counted in NVS, first thing, see count, and logged on the SD card, if there is one, once the time is
//! known, see record, so that starts which were not logged, e.g. as the device restarted before it was on the Wi-Fi,
//! show as jumps in the count.

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::sync::OnceLock;

use chrono::{SecondsFormat, TimeDelta, Utc};
use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use log::{info, warn};
use serde_json::{Value, json};

use crate::Firmware;

/// The NVS namespace, the one the device's identity is in, see identity.rs.
const NAMESPACE: &str = "device";
const COUNT_KEY: &str = "start_count";
/// How many of the latest starts /api/starts gives.
const MAX_STARTS: usize = 100;

/// This start's number, see count.
static COUNT: OnceLock<u32> = OnceLock::new();

/// Counts this start in NVS, which must be done once, as early as possible, so that starts the device does not get as
/// far as logging are counted.
pub fn count(nvs_partition: &EspNvsPartition<NvsDefault>) {
    let result = (|| -> anyhow::Result<u32> {
        let nvs = EspNvs::new(nvs_partition.clone(), NAMESPACE, true)?;
        let count = nvs.get_u32(COUNT_KEY)?.unwrap_or_default().wrapping_add(1);
        nvs.set_u32(COUNT_KEY, count)?;
        Ok(count)
    })();
    match result {
        Ok(count) => {
            info!("Start {count}, after {}", crate::reset_reason());
            let _ = COUNT.set(count);
        }
        Err(error) => warn!("Unable to count the start, {error:#}"),
    }
}

/// Where the log is, on the SD card mounted at sd_card.
fn path(sd_card: &str) -> String {
    format!("{sd_card}/device/starts.jsonl")
}

/// This start, as it is logged: when the device started, why, its number, see count, the firmware, and how long it
/// took to get the time, after which the device can do anything useful.
fn this_start(firmware: Firmware) -> Value {
    let up_time = TimeDelta::microseconds(unsafe { esp_idf_sys::esp_timer_get_time() });
    json!({
        "time": (Utc::now() - up_time).to_rfc3339_opts(SecondsFormat::Secs, true),
        "reason": crate::reset_reason(),
        "count": COUNT.get(),
        "firmware": format!("{} {}", firmware.name, firmware.version),
        "platform": format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        "secondsToTime": up_time.num_seconds(),
    })
}

/// Logs this start on the SD card mounted at sd_card, which must be done once the time is known, and before anything
/// which might make the device restart, e.g. the features' first polls.
pub fn record(sd_card: &str, firmware: Firmware) {
    let result = (|| -> anyhow::Result<()> {
        let path = path(sd_card);
        std::fs::create_dir_all(format!("{sd_card}/device"))?;
        let mut line = serde_json::to_vec(&this_start(firmware))?;
        line.push(b'\n');
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(&line)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!("Unable to log the start on the SD card, {error:#}");
    }
}

/// The latest starts, oldest first, up to MAX_STARTS, from the log on the SD card mounted at sd_card, or if there is
/// none, or it cannot be read, only this one, with why not, as {"count": this start's number, "starts": [...],
/// "problem": ...}.
pub fn to_json(sd_card: Result<&str, String>, firmware: Firmware) -> Value {
    let read = |sd_card: &str| -> anyhow::Result<Vec<Value>> {
        let file = std::fs::File::open(path(sd_card))?;
        let mut starts = VecDeque::with_capacity(MAX_STARTS);
        for line in std::io::BufReader::with_capacity(1024, file).lines() {
            // A line cut short by a power cut is left out.
            let Ok(start) = serde_json::from_str::<Value>(&line?) else { continue };
            if starts.len() == MAX_STARTS {
                starts.pop_front();
            }
            starts.push_back(start);
        }
        Ok(starts.into())
    };
    let (starts, problem) = match sd_card.map_err(anyhow::Error::msg).and_then(read) {
        Ok(starts) => (starts, None),
        Err(error) => (vec![this_start(firmware)], Some(format!("{error:#}"))),
    };
    json!({ "count": COUNT.get(), "starts": starts, "problem": problem })
}
