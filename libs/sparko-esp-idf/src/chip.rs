//! What the chip is, read from the chip itself: its model, e.g. ESP32-C6, its silicon revision, its cores, and the
//! flash and PSRAM it has, for /api/info, see api.rs, and the /config page. Which board it is on cannot be read from
//! the hardware, only which one the firmware was built for, see board_name.

use serde_json::{Value, json};

/// ESP-IDF's esp_chip_info_t, from esp_chip_info.h, which esp-idf-sys does not generate bindings for.
#[repr(C)]
#[derive(Default)]
struct EspChipInfo {
    model: u32,
    features: u32,
    /// The major revision times 100 plus the minor, e.g. 301 for v3.1.
    revision: u16,
    cores: u8,
}

unsafe extern "C" {
    fn esp_chip_info(out_info: *mut EspChipInfo);
}

pub struct Chip {
    /// e.g. ESP32-C6, or the model's number if it is one this does not know.
    pub model: String,
    /// e.g. v0.2.
    pub revision: String,
    pub cores: u8,
    pub flash_bytes: u64,
    pub psram_bytes: u64,
}

/// The chip, as it says it is.
pub fn chip() -> Chip {
    let mut info = EspChipInfo::default();
    unsafe { esp_chip_info(&mut info) };
    // esp_chip_model_t's values.
    let model = match info.model {
        1 => "ESP32",
        2 => "ESP32-S2",
        5 => "ESP32-C3",
        9 => "ESP32-S3",
        12 => "ESP32-C2",
        13 => "ESP32-C6",
        16 => "ESP32-H2",
        18 => "ESP32-P4",
        20 => "ESP32-C61",
        23 => "ESP32-C5",
        other => return Chip { model: format!("model {other}"), ..chip_details(&info) },
    };
    Chip { model: model.to_string(), ..chip_details(&info) }
}

/// The chip's details besides its model.
fn chip_details(info: &EspChipInfo) -> Chip {
    let mut flash = 0u32;
    if unsafe { esp_idf_sys::esp_flash_get_size(std::ptr::null_mut(), &mut flash) } != esp_idf_sys::ESP_OK {
        flash = 0;
    }
    Chip {
        model: String::new(),
        revision: format!("v{}.{}", info.revision / 100, info.revision % 100),
        cores: info.cores,
        flash_bytes: flash as u64,
        psram_bytes: unsafe { esp_idf_sys::heap_caps_get_total_size(esp_idf_sys::MALLOC_CAP_SPIRAM) } as u64,
    }
}

impl Chip {
    pub fn to_json(&self) -> Value {
        json!({
            "model": self.model,
            "revision": self.revision,
            "cores": self.cores,
            "flashBytes": self.flash_bytes,
            "psramBytes": self.psram_bytes,
        })
    }

    /// e.g. ESP32-C6 v0.2, 1 core, 8 MB flash, no PSRAM.
    pub fn describe(&self) -> String {
        let mb = |bytes: u64| format!("{} MB", bytes / (1024 * 1024));
        let cores = if self.cores == 1 { "1 core".to_string() } else { format!("{} cores", self.cores) };
        let psram = if self.psram_bytes == 0 { "no PSRAM".to_string() } else { format!("{} PSRAM", mb(self.psram_bytes)) };
        format!("{} {}, {cores}, {} flash, {psram}", self.model, self.revision, mb(self.flash_bytes))
    }
}
