//! The board's battery, e.g. the Waveshare ESP32-S3-RLCD-4.2's 18650 cell, whose voltage, through a divider which
//! makes it a third, is read on an ADC pin, GPIO4, ADC1's channel 3, as Waveshare's 03_ADC_Test example reads it, see
//! Esp32Platform::battery.
//!
//! The voltage says roughly how charged the cell is, see percent, but not exactly: it is higher while it charges and
//! lower under load, and with no cell at all the charger's own voltage is read on USB, which looks like a full one.

use std::sync::Mutex;

use esp_idf_hal::adc::oneshot::config::{AdcChannelConfig, Calibration};
use esp_idf_hal::adc::oneshot::{AdcChannelDriver, AdcDriver};
use esp_idf_hal::gpio::Gpio4;
use esp_idf_hal::adc::ADC1;
use esp_idf_sys::EspError;

/// How much the divider divides the cell's voltage by.
const DIVIDER: f32 = 3.0;
/// How many readings each is the average of, as one is noisy.
const SAMPLES: u32 = 8;

/// The battery's voltage and how full that says it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatteryReading {
    pub volts: f32,
    pub percent: u8,
}

/// The battery's ADC channel, see the module comment.
pub struct Battery {
    read_millivolts: Mutex<Box<dyn FnMut() -> Result<u16, EspError> + Send>>,
}

impl Battery {
    pub(crate) fn new(adc: ADC1<'static>, pin: Gpio4<'static>) -> anyhow::Result<Battery> {
        let driver = AdcDriver::new(adc)?;
        // Up to about 3.1 V at the pin, a cell's 4.2 V being 1.4 V there, calibrated to mV.
        let config = AdcChannelConfig {
            attenuation: esp_idf_hal::adc::attenuation::DB_12,
            calibration: Calibration::Curve,
            ..AdcChannelConfig::new()
        };
        let mut channel = AdcChannelDriver::new(driver, pin, &config)?;
        Ok(Battery { read_millivolts: Mutex::new(Box::new(move || channel.read())) })
    }

    /// The battery's voltage, the average of SAMPLES readings, to a hundredth of a volt, and how full that says it is.
    pub fn read(&self) -> anyhow::Result<BatteryReading> {
        let mut read = self.read_millivolts.lock().unwrap();
        let mut total = 0u32;
        for _ in 0..SAMPLES {
            total += read()? as u32;
        }
        // To the hundredth of a volt which is shown, and how full it is from that, so that the two only change together.
        let volts = (total as f32 / SAMPLES as f32 / 1000.0 * DIVIDER * 100.0).round() / 100.0;
        Ok(BatteryReading { volts, percent: percent(volts) })
    }
}

/// A lithium ion cell's charge at its voltage at rest, roughly, as it is far from linear: most of it is between 3.7 V
/// and 4.0 V.
const CHARGE_CURVE: [(f32, u8); 12] = [
    (3.00, 0),
    (3.30, 5),
    (3.55, 10),
    (3.65, 20),
    (3.70, 30),
    (3.75, 40),
    (3.80, 50),
    (3.85, 60),
    (3.92, 70),
    (4.00, 80),
    (4.10, 90),
    (4.20, 100),
];

/// How full a cell of the voltage is, between the points of CHARGE_CURVE.
pub fn percent(volts: f32) -> u8 {
    let (first, last) = (CHARGE_CURVE[0], CHARGE_CURVE[CHARGE_CURVE.len() - 1]);
    if volts <= first.0 {
        return first.1;
    }
    if volts >= last.0 {
        return last.1;
    }
    CHARGE_CURVE.windows(2)
        .find(|pair| volts < pair[1].0)
        .map(|pair| {
            let ((low_volts, low), (high_volts, high)) = (pair[0], pair[1]);
            (low as f32 + (volts - low_volts) / (high_volts - low_volts) * (high - low) as f32).round() as u8
        })
        .unwrap_or(last.1)
}
