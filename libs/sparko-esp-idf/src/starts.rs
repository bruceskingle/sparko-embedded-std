//! A log of the device's starts, and why each happened, e.g. Panic, or Brownout, see reset_reason, as it only knows
//! why it last started, so that restarts in the night, which only show as gaps in what it captured, can be seen, from
//! GET /api/starts, see to_json.
//!
//! Each start is counted in NVS, first thing, see count, and logged in NVS too, once the time is known, see record, so
//! that starts which were not logged, e.g. as the device restarted before it was on the Wi-Fi, show as jumps in the
//! count. The log is kept in NVS, not on an SD card, so that every device has one, and it is the device's own: a card
//! moved to another device took its log with it, which then showed as that device's.
//!
//! The log is the latest MAX_STARTS starts, as one blob of JSON lines, a few KB, which NVS spreads its writes of, as
//! it does every key's, so that a device which restarts again and again does not wear out its flash.

use std::sync::OnceLock;

use chrono::{SecondsFormat, TimeDelta, Utc};
use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use log::{info, warn};
use serde_json::value::RawValue;
use serde_json::{Value, json};

use crate::Firmware;

/// The NVS namespace, the one the device's identity is in, see identity.rs.
const NAMESPACE: &str = "device";
const COUNT_KEY: &str = "start_count";
const LOG_KEY: &str = "start_log";
/// How many of the latest starts are kept, and /api/starts gives.
const MAX_STARTS: usize = 30;

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

/// The log, its JSON lines, oldest first, or none if nothing has been logged yet.
fn read_log(nvs: &EspNvs<NvsDefault>) -> anyhow::Result<Vec<u8>> {
    let Some(length) = nvs.blob_len(LOG_KEY)? else { return Ok(Vec::new()) };
    let mut log = vec![0u8; length];
    let read = nvs.get_blob(LOG_KEY, &mut log)?.map_or(0, |read| read.len());
    log.truncate(read);
    Ok(log)
}

/// The log's lines, leaving out any which are not JSON.
fn lines(log: &[u8]) -> impl Iterator<Item = &[u8]> {
    log.split(|byte| *byte == b'\n').filter(|line| serde_json::from_slice::<&RawValue>(line).is_ok())
}

/// Logs this start in NVS, keeping the latest MAX_STARTS, which must be done once the time is known, and before
/// anything which might make the device restart, e.g. the features' first polls.
pub fn record(nvs_partition: &EspNvsPartition<NvsDefault>, firmware: Firmware) {
    let result = (|| -> anyhow::Result<()> {
        let nvs = EspNvs::new(nvs_partition.clone(), NAMESPACE, true)?;
        let log = read_log(&nvs)?;
        let this = serde_json::to_vec(&this_start(firmware))?;
        let kept: Vec<&[u8]> = lines(&log).collect();
        let mut updated = Vec::with_capacity(log.len() + this.len() + 1);
        for line in kept.iter().skip((kept.len() + 1).saturating_sub(MAX_STARTS)) {
            updated.extend_from_slice(line);
            updated.push(b'\n');
        }
        updated.extend_from_slice(&this);
        nvs.set_blob(LOG_KEY, &updated)?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!("Unable to log the start, {error:#}");
    }
}

/// Writes, with write, the logged starts, oldest first, or if the log cannot be read, only this one, with why not, as
/// the JSON of {"count": this start's number, "problem": ..., "starts": [...]}.
pub fn write_json(
    nvs_partition: &EspNvsPartition<NvsDefault>,
    firmware: Firmware,
    write: &mut dyn FnMut(&[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let log = EspNvs::new(nvs_partition.clone(), NAMESPACE, false).map_err(anyhow::Error::from).and_then(|nvs| read_log(&nvs));
    let problem = log.as_ref().err().map(|error| format!("{error:#}"));
    write(format!(r#"{{"count":{},"problem":{},"starts":["#, json!(COUNT.get()), json!(problem)).as_bytes())?;
    match &log {
        Ok(log) => {
            for (index, line) in lines(log).enumerate() {
                if index > 0 {
                    write(b",")?;
                }
                write(line)?;
            }
        }
        Err(_) => write(&serde_json::to_vec(&this_start(firmware))?)?,
    }
    write(b"]}")
}
