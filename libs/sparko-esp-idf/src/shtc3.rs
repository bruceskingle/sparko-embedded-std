//! The Sensirion SHTC3 temperature and humidity sensor, on the board's I2C bus, e.g. the Waveshare ESP32-S3-RLCD-4.2's,
//! which it shares with the board's other chips, see Esp32Platform::shtc3. Between readings it sleeps, using almost
//! nothing. Each reading wakes it, measures in normal mode, which takes up to 12 ms, checks the readings' CRCs, and
//! puts it to sleep again, as its datasheet says.
//!
//! Being on the board, it is warmed by it, so each reading is corrected by the offset the TemperatureHumidity feature
//! sets, and its humidity with it: the air's moisture is the same, but relative humidity is lower where it is warmer,
//! so it is worked out again for the corrected temperature.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::bail;
use esp_idf_hal::delay::BLOCK;
use esp_idf_hal::i2c::I2cDriver;

const ADDRESS: u8 = 0x70;
const WAKE: [u8; 2] = [0x35, 0x17];
/// Normal mode, temperature first, without clock stretching, so that the bus is free while it measures.
const MEASURE: [u8; 2] = [0x78, 0x66];
const SLEEP: [u8; 2] = [0xB0, 0x98];

/// Added to every reading's temperature, in tenths of a degree, see set_offset_tenths.
static OFFSET_TENTHS: AtomicI32 = AtomicI32::new(0);

/// Sets the offset, from the TemperatureHumidity feature's settings.
pub fn set_offset_tenths(offset: i32) {
    OFFSET_TENTHS.store(offset, Ordering::Relaxed);
}

/// A reading, in °C and % relative humidity, corrected by the offset, and as the sensor gave it.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub celsius: f32,
    pub humidity: f32,
    pub sensor_celsius: f32,
    pub sensor_humidity: f32,
}

#[derive(Clone)]
pub struct Shtc3 {
    i2c: Arc<Mutex<I2cDriver<'static>>>,
}

impl Shtc3 {
    pub fn new(i2c: Arc<Mutex<I2cDriver<'static>>>) -> Self {
        Shtc3 { i2c }
    }

    /// Measures the temperature and humidity, holding the bus throughout, about 15 ms.
    pub fn read(&self) -> anyhow::Result<Reading> {
        let mut i2c = self.i2c.lock().unwrap();
        i2c.write(ADDRESS, &WAKE, BLOCK)?;
        // It takes up to 240 µs to wake.
        std::thread::sleep(Duration::from_millis(1));
        i2c.write(ADDRESS, &MEASURE, BLOCK)?;
        std::thread::sleep(Duration::from_millis(13));
        let mut data = [0u8; 6];
        i2c.read(ADDRESS, &mut data, BLOCK)?;
        i2c.write(ADDRESS, &SLEEP, BLOCK)?;
        let temperature = word(&data[0..3])?;
        let humidity = word(&data[3..6])?;
        let sensor_celsius = -45.0 + 175.0 * temperature as f32 / 65536.0;
        let sensor_humidity = 100.0 * humidity as f32 / 65536.0;
        let celsius = sensor_celsius + OFFSET_TENTHS.load(Ordering::Relaxed) as f32 / 10.0;
        Ok(Reading { celsius, humidity: corrected_humidity(sensor_celsius, sensor_humidity, celsius), sensor_celsius, sensor_humidity })
    }
}

/// The relative humidity at celsius of air whose relative humidity is humidity at sensor_celsius: the same moisture,
/// over the saturation vapour pressure at each, from the Magnus formula, with Sensirion's constants.
fn corrected_humidity(sensor_celsius: f32, humidity: f32, celsius: f32) -> f32 {
    let saturation = |celsius: f32| (17.62 * celsius / (243.12 + celsius)).exp();
    (humidity * saturation(sensor_celsius) / saturation(celsius)).clamp(0.0, 100.0)
}

/// A reading's 16 bits, from its two bytes and their CRC.
fn word(bytes: &[u8]) -> anyhow::Result<u16> {
    if crc(&bytes[..2]) != bytes[2] {
        bail!("the SHTC3's reading failed its CRC");
    }
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// CRC-8, polynomial 0x31, from 0xFF, as the SHTC3 calculates it.
fn crc(bytes: &[u8]) -> u8 {
    let mut crc = 0xFFu8;
    for byte in bytes {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x31 } else { crc << 1 };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_is_the_datasheets() {
        // The SHTC3 datasheet's example: 0xBEEF gives 0x92.
        assert_eq!(super::crc(&[0xBE, 0xEF]), 0x92);
    }
}
