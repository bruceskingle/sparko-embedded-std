//! The platform on the Waveshare ESP32-S3-RLCD-4.2: an analog clock on its reflective display, a test pattern of the
//! display's edges, see edge_test.rs, to enable on the /config page in place of the clock when needed, and a message
//! when the KEY button is pressed, of the SD card's size and free space, or BOOT, of the temperature and humidity, see
//! message.rs, each with a beep on the speaker, and a chime as it starts, sounds/Startup.wav, built into the firmware.

mod edge_test;
mod message;

use std::time::Duration;

use log::error;
use sparko_esp_idf::{
    Esp32Platform,
    features::analog_clock::AnalogClock,
};

/// Played as the board starts.
static STARTUP_SOUND: &[u8] = include_bytes!("../sounds/Startup.wav");

fn main() -> anyhow::Result<()> {
    let (builder, _remainder) = Esp32Platform::builder()?;

    let platform = builder
        .with_feature(Box::new(AnalogClock::builder().build()?))?
        .with_feature(Box::new(edge_test::EdgeTest))?
        // The platform owns the buttons, see the key-button and boot-button features, and this says what a press does.
        // KEY shows the SD card's size and free space, or why there is no card.
        .with_key_button(|sparko| {
            let display_manager = sparko.display_manager.clone();
            let speaker = sparko.speaker();
            let sd_card = sparko.sd_card();
            Ok(Box::new(move || {
                speaker.play_tone(880.0, Duration::from_millis(120))?;
                let text = match (&sd_card, sparko_esp_idf::sd_card::space()) {
                    (Ok(_), Some((total, free))) => format!("SD {:.1} GB, {:.1} GB free", total as f64 / 1e9, free as f64 / 1e9),
                    (Ok(_), None) => "SD card cannot be read".to_string(),
                    (Err(error), _) => format!("No SD card: {error}"),
                };
                message::show(&display_manager, &text)
            }))
        })
        // BOOT shows the temperature and humidity.
        .with_boot_button(|sparko| {
            let display_manager = sparko.display_manager.clone();
            let speaker = sparko.speaker();
            let sensor = sparko.shtc3();
            Ok(Box::new(move || {
                speaker.play_tone(660.0, Duration::from_millis(100))?;
                speaker.play_tone(990.0, Duration::from_millis(150))?;
                let text = match sensor.read() {
                    Ok(reading) => format!("{:.1} C, {:.0}% humidity", reading.celsius, reading.humidity),
                    Err(error) => format!("No temperature: {error:#}"),
                };
                message::show(&display_manager, &text)
            }))
        })
        .with_firmware(sparko_esp_idf::firmware!())
        .build()?;

    // At once, while the board joins the network, so in a thread of its own, at the volume set on the /config page.
    let speaker = platform.sparko_std.speaker();
    std::thread::Builder::new().stack_size(8192).spawn(move || {
        if let Err(error) = speaker.play_wav(STARTUP_SOUND) {
            error!("Unable to play the startup sound: {error:#}");
        }
    })?;

    platform.start()
}
