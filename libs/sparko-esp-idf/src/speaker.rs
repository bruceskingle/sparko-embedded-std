//! The board's speaker, e.g. the Waveshare ESP32-S3-RLCD-4.2's: an Everest ES8311 codec, set up over the board's I2C
//! bus, given 16-bit audio over I2S, whose clocks the ESP32 drives, and an amplifier, which is only on while playing,
//! so that it neither hisses nor uses power otherwise. See Esp32Platform::speaker.
//!
//! The ES8311's registers are set as Espressif's esp_codec_dev driver sets them, for its DAC only, with MCLK at 256
//! times the sample rate, which the ESP32 gives it on its MCLK pin.
//!
//! It plays sounds made with tone, or from WAV files, see wav_samples.

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use esp_idf_hal::delay::BLOCK;
use esp_idf_hal::gpio::{Output, PinDriver};
use esp_idf_hal::i2c::I2cDriver;
use esp_idf_hal::i2s::{I2sDriver, I2sTx};
use anyhow::bail;
use log::info;

/// The rate of the audio play takes, in samples a second.
pub const SAMPLE_RATE: u32 = 16_000;
const ES8311: u8 = 0x18;
/// The ES8311's start-up, each register and its value: reset, clocks from the MCLK pin, dividers for MCLK at 256 times
/// the sample rate, I2S of 16 bits, and the DAC and its output powered up.
const ES8311_INIT: &[(u8, u8)] = &[
    (0x01, 0x30),
    (0x02, 0x00),
    (0x03, 0x10),
    (0x16, 0x24),
    (0x04, 0x10),
    (0x05, 0x00),
    (0x0B, 0x00),
    (0x0C, 0x00),
    (0x10, 0x1F),
    (0x11, 0x7F),
    (0x00, 0x80), // Powered on, slave to the ESP32's clocks
    (0x01, 0x3F), // All clocks on, MCLK from its pin
    (0x02, 0x00), // MCLK not divided or multiplied
    (0x05, 0x00),
    (0x03, 0x10), // Single speed
    (0x04, 0x10),
    (0x07, 0x00), // LRCK is MCLK over 256
    (0x08, 0xFF),
    (0x06, 0x03),
    (0x13, 0x10),
    (0x1B, 0x0A),
    (0x1C, 0x6A),
    (0x09, 0x0C), // DAC's input: I2S, 16 bits
    (0x0A, 0x0C), // ADC's output: I2S, 16 bits
    (0x17, 0xBF),
    (0x0E, 0x02),
    (0x12, 0x00), // DAC powered up
    (0x14, 0x1A),
    (0x0D, 0x01), // Analogue powered up
    (0x15, 0x40),
    (0x37, 0x08), // DAC's equaliser bypassed
    (0x45, 0x00),
    (0x31, 0x00), // DAC not muted
];
/// The DAC's volume register, 0 for -95.5 dB, in half dBs, up to 0xBF for 0 dB.
const VOLUME: u8 = 0x32;
/// Silence written after the audio, so that the I2S sends it while the amplifier turns off: this many blocks of
/// SILENCE, about 100 ms. Written as blocks, not added to the audio, as the Xtensa compiler crashed on the sum of their
/// sizes ("Cannot select: Constant<6400>").
const SILENT_BLOCKS: usize = 6;
/// The samples play sends to the I2S at a time.
const BLOCK_SAMPLES: usize = 1024;
static SILENCE: [u8; 1024] = [0; 1024];
/// How long the amplifier takes to turn on.
const AMPLIFIER_DELAY: Duration = Duration::from_millis(10);
/// Each tone fades in and out over this long, so that it does not click.
const FADE: Duration = Duration::from_millis(5);

struct Inner {
    i2c: Arc<Mutex<I2cDriver<'static>>>,
    i2s: I2sDriver<'static, I2sTx>,
    amplifier: PinDriver<'static, Output>,
}

/// A handle on the speaker, which can be cloned. Each play takes it for as long as it lasts.
#[derive(Clone)]
pub struct Speaker {
    inner: Arc<Mutex<Inner>>,
    /// In percent, see set_volume.
    volume: Arc<AtomicU8>,
}

impl Speaker {
    /// Sets up the codec on i2c, and starts the I2S output, with the amplifier off.
    pub fn new(i2c: Arc<Mutex<I2cDriver<'static>>>, mut i2s: I2sDriver<'static, I2sTx>, mut amplifier: PinDriver<'static, Output>) -> anyhow::Result<Self> {
        amplifier.set_low()?;
        {
            let mut bus = i2c.lock().unwrap();
            bus.write(ES8311, &[0x00, 0x1F], BLOCK)?; // Reset
            std::thread::sleep(Duration::from_millis(20));
            bus.write(ES8311, &[0x00, 0x00], BLOCK)?;
            for (register, value) in ES8311_INIT {
                bus.write(ES8311, &[*register, *value], BLOCK)?;
            }
        }
        i2s.tx_enable()?;
        info!("Speaker started");
        Ok(Speaker { inner: Arc::new(Mutex::new(Inner { i2c, i2s, amplifier })), volume: Arc::new(AtomicU8::new(100)) })
    }

    /// Sets the volume every sound plays at, in percent, see level, from the Speaker feature's settings.
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100), Ordering::Relaxed);
    }

    /// Plays samples, 16-bit mono at SAMPLE_RATE, at the volume, waiting until they have been sent.
    pub fn play(&self, samples: &[i16]) -> anyhow::Result<()> {
        self.play_at(self.volume.load(Ordering::Relaxed), samples)
    }

    /// Plays samples as play does, but at volume, in percent, not the volume set, e.g. to hear a volume before it is
    /// saved, see the Speaker feature's slider.
    pub fn play_at(&self, volume: u8, samples: &[i16]) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let level = level(volume);
        inner.i2c.lock().unwrap().write(ES8311, &[VOLUME, level], BLOCK)?;
        inner.amplifier.set_high()?;
        std::thread::sleep(AMPLIFIER_DELAY);
        // Each sample to both channels, little endian, a block at a time, so that a long sound needs no large buffer.
        let mut frames = Vec::with_capacity(BLOCK_SAMPLES * 4);
        let mut result = Ok(());
        for block in samples.chunks(BLOCK_SAMPLES) {
            frames.clear();
            for sample in block {
                let bytes = sample.to_le_bytes();
                frames.extend_from_slice(&[bytes[0], bytes[1], bytes[0], bytes[1]]);
            }
            result = inner.i2s.write_all(&frames, BLOCK);
            if result.is_err() {
                break;
            }
        }
        for _ in 0..SILENT_BLOCKS {
            if result.is_ok() {
                result = inner.i2s.write_all(&SILENCE, BLOCK);
            }
        }
        inner.amplifier.set_low()?;
        Ok(result?)
    }

    /// Plays a WAV file's audio, e.g. one built into the firmware with include_bytes!, at the volume, see wav_samples.
    pub fn play_wav(&self, wav: &[u8]) -> anyhow::Result<()> {
        self.play(&wav_samples(wav)?)
    }

    /// Plays a tone of frequency, in Hz, for duration, at the volume.
    pub fn play_tone(&self, frequency: f32, duration: Duration) -> anyhow::Result<()> {
        self.play(&tone(frequency, duration))
    }
}

/// A WAV file's audio, 16-bit PCM of any rate and number of channels, as play takes it: its channels mixed into one,
/// and resampled to SAMPLE_RATE, by linear interpolation, which is good enough for chimes and alerts.
pub fn wav_samples(wav: &[u8]) -> anyhow::Result<Vec<i16>> {
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        bail!("not a WAV file");
    }
    let mut format = None;
    let mut data = None;
    let mut chunks = &wav[12..];
    while chunks.len() >= 8 {
        let size = u32::from_le_bytes([chunks[4], chunks[5], chunks[6], chunks[7]]) as usize;
        let body = &chunks[8..(8 + size).min(chunks.len())];
        match &chunks[0..4] {
            b"fmt " if body.len() >= 16 => format = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        // Chunks are padded to an even size.
        chunks = &chunks[(8 + size + size % 2).min(chunks.len())..];
    }
    let (Some(format), Some(data)) = (format, data) else { bail!("the WAV file has no format or data") };
    let encoding = u16::from_le_bytes([format[0], format[1]]);
    let channels = u16::from_le_bytes([format[2], format[3]]) as usize;
    let rate = u32::from_le_bytes([format[4], format[5], format[6], format[7]]);
    let bits = u16::from_le_bytes([format[14], format[15]]);
    // 0xFFFE, WAVE_FORMAT_EXTENSIBLE, is PCM too, when its bits say 16.
    if !(encoding == 1 || encoding == 0xFFFE) || bits != 16 || channels == 0 || rate == 0 {
        bail!("the WAV file is not 16-bit PCM (format {encoding}, {bits} bits, {channels} channels, {rate} Hz)");
    }
    let mono: Vec<i16> = data
        .chunks_exact(2 * channels)
        .map(|frame| {
            let sum: i32 = frame.chunks_exact(2).map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as i32).sum();
            (sum / channels as i32) as i16
        })
        .collect();
    if rate == SAMPLE_RATE || mono.is_empty() {
        return Ok(mono);
    }
    let step = rate as f32 / SAMPLE_RATE as f32;
    let count = ((mono.len() - 1) as f32 / step) as usize + 1;
    Ok((0..count)
        .map(|index| {
            let position = index as f32 * step;
            let before = position as usize;
            let after = (before + 1).min(mono.len() - 1);
            let fraction = position - before as f32;
            (mono[before] as f32 + (mono[after] as f32 - mono[before] as f32) * fraction) as i16
        })
        .collect())
}

/// The DAC's volume register for volume, in percent: 100 is 0 dB, the codec's full level, each percent less is half a
/// dB quieter, as loudness is heard in dBs, e.g. 80 is -10 dB, and 0 is silent.
fn level(volume: u8) -> u8 {
    match volume.min(100) {
        0 => 0,
        volume => 0xBF - (100 - volume),
    }
}

/// A sine wave of frequency for duration, at nearly full scale, fading in and out over FADE.
pub fn tone(frequency: f32, duration: Duration) -> Vec<i16> {
    let count = (duration.as_secs_f32() * SAMPLE_RATE as f32) as usize;
    let fade = ((FADE.as_secs_f32() * SAMPLE_RATE as f32) as usize).max(1);
    (0..count)
        .map(|index| {
            let envelope = (index.min(count - 1 - index) as f32 / fade as f32).min(1.0);
            let phase = TAU * frequency * index as f32 / SAMPLE_RATE as f32;
            (phase.sin() * envelope * i16::MAX as f32 * 0.9) as i16
        })
        .collect()
}
