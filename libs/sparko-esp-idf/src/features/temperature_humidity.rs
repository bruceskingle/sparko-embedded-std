//! The board's temperature and humidity sensor's settings, and a reading in the log every minute: the platform adds it
//! on a board with one, see the shtc3 feature. The sensor is on the board, which warms it, so it reads high, by how
//! much depending on the board and what it is doing: the offset corrects it, see shtc3.rs, set by comparing with a
//! thermometer which can be trusted, after the board has run for a while.

use log::info;
use sparko_embedded_std::config::{ConfigSpec, ConfigSpecValue, FeatureConfig, TypedValue};
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;

use crate::{Esp32Platform, Esp32PlatformInitializer, Feature, FeatureDescriptor};

#[derive(FeatureConfig)]
#[config(label = "Temperature and Humidity")]
pub struct TemperatureHumidityConfig {
    /// Added to the sensor's temperature, in tenths of a degree, e.g. -63 for -6.3 °C. None if not set.
    #[config(label = "Temperature Offset (tenths of °C)")]
    pub offset_tenths: Option<i32>,
}

/// Named TempHumidity, as names are keys in NVS, which allows no more than 15 characters.
pub struct TemperatureHumidity;

impl Feature for TemperatureHumidity {
    type Config = TemperatureHumidityConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor { name: "TempHumidity".to_string(), config: TemperatureHumidityConfig::to_config_spec()? })
    }

    fn start(&mut self, sparko: &mut Esp32Platform, initializer: &mut Esp32PlatformInitializer, config: TemperatureHumidityConfig) -> anyhow::Result<()> {
        crate::shtc3::set_offset_tenths(config.offset_tenths.unwrap_or(0));
        initializer.add_task(Box::new(LogTask { sensor: sparko.shtc3() }), "0 * * * * *")?;
        Ok(())
    }
}

/// Logs a reading, corrected and as the sensor gave it, so that how the board warms can be seen, and the offset set.
struct LogTask {
    sensor: crate::shtc3::Shtc3,
}

impl ScheduledTask<Esp32Platform> for LogTask {
    fn name(&self) -> &str {
        "Temperature and Humidity"
    }

    fn run(&mut self, _sparko: &mut Esp32Platform) -> anyhow::Result<()> {
        let reading = self.sensor.read()?;
        info!("Temperature {:.1} °C, humidity {:.0}% (sensor {:.1} °C, {:.0}%)", reading.celsius, reading.humidity,
            reading.sensor_celsius, reading.sensor_humidity);
        Ok(())
    }
}
