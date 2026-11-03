//! A log of the device's starts, and why each happened, e.g. Panic, or Brownout, see reset_reason, as it only knows
//! why it last started, so that restarts in the night, which only show as gaps in what it captured, can be seen, from
//! GET /api/starts, see to_json.
//!
//! Each start is counted in NVS, first thing, see count, and logged on the SD card, if there is one, once the time is
//! known, see record, so that starts which were not logged, e.g. as the device restarted before it was on the Wi-Fi,
//! show as jumps in the count.
//!
//! The starts are given as they are in the log, a line at a time, see write_json, not as serde_json::Values, each of
//! which is a BTreeMap of at least 400 bytes, so that the log of a device which restarts again and again does not run
//! it out of memory when it is read.

use std::io::{BufRead, Write};
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

/// Writes, with write, the latest starts, oldest first, up to MAX_STARTS, from the log on the SD card mounted at
/// sd_card, or if there is none, or it cannot be read, only this one, with why not, as the JSON of {"count": this
/// start's number, "problem": ..., "starts": [...]}. The log is read twice, to count its lines, then to write the
/// latest, a line at a time, as the latest starts of a device which restarts again and again do not fit in the RAM
/// with the HTTPS connection they are given on.
pub fn write_json(
    sd_card: Result<&str, String>,
    firmware: Firmware,
    write: &mut dyn FnMut(&[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let open = |sd_card: &str| std::fs::File::open(path(sd_card)).map(|file| std::io::BufReader::with_capacity(1024, file));
    let lines = sd_card.map_err(anyhow::Error::msg).and_then(|sd_card| {
        let count = open(sd_card)?.lines().try_fold(0usize, |count, line| line.map(|_| count + 1))?;
        Ok((open(sd_card)?, count))
    });
    let problem = lines.as_ref().err().map(|error| format!("{error:#}"));
    write(format!(r#"{{"count":{},"problem":{},"starts":["#, json!(COUNT.get()), json!(problem)).as_bytes())?;
    match lines {
        Ok((file, count)) => {
            let mut first = true;
            for line in file.lines().skip(count.saturating_sub(MAX_STARTS)) {
                let line = line?;
                // A line cut short by a power cut is left out.
                if serde_json::from_str::<&RawValue>(&line).is_err() {
                    continue;
                }
                if !first {
                    write(b",")?;
                }
                first = false;
                write(line.as_bytes())?;
            }
        }
        Err(_) => write(&serde_json::to_vec(&this_start(firmware))?)?,
    }
    write(b"]}")
}
