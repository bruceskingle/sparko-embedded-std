use crate::Esp32Platform;
use crate::Esp32PlatformInitializer;
use crate::smart_led::SmartLeds;
use crate::smart_led::SmartLedsRmt;
use crate::smart_led::SmartLedsSpi;
use crate::{Feature, FeatureDescriptor};
use chrono::Datelike;
use chrono::Local;
use chrono::Timelike;
use esp_idf_hal::spi::SpiDriver;
use log::info;
use rgb::RGB8;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config::FeatureConfig;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;
use std::borrow::Borrow;

const HIGH: u8 = 16;
const LOW: u8 = 4;
const MIN: u8 = 0;

pub struct BinaryDigitConfig {
    off_color: rgb::RGB<u8>,
    on_color: rgb::RGB<u8>,
    bits: Vec<usize>,
}

#[derive(FeatureConfig)]
pub struct BinaryClockConfig {}

pub struct BinaryClockLayout {
    digits: Vec<BinaryDigitConfig>,
}

impl BinaryClockLayout {
    pub fn default() -> Self {
        Self {
            digits: vec![
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, 0),
                    on_color: RGB8::new(HIGH, 0, 0),
                    bits: vec![0, 1],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, 0),
                    on_color: RGB8::new(HIGH, LOW, LOW),
                    bits: vec![2, 3, 4, 5],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, 0),
                    on_color: RGB8::new(0, HIGH, 0),
                    bits: vec![6, 7, 8],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, 0),
                    on_color: RGB8::new(LOW, HIGH, LOW),
                    bits: vec![9, 10, 11, 12],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, 0),
                    on_color: RGB8::new(0, 0, HIGH),
                    bits: vec![13, 14, 15],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, 0),
                    on_color: RGB8::new(LOW, LOW, HIGH),
                    bits: vec![16, 17, 18, 19],
                },
            ],
        }
    }

    /*
       8 x 8 matrix layout, snake wired
       63, 62, 61, 60, 59, 58, 57, 56,
       48, 49, 50, 51, 52, 53, 54, 55,
       47, 46, 45, 44, 43, 42, 41, 40,
       32, 33, 34, 35, 36, 37, 38, 39,

       31, 30, 29, 28, 27, 26, 25, 24,
       16, 17, 18, 19, 20, 21, 22, 23,
       15, 14, 13, 12, 11, 10, 9,  8,
        0, 1,  2,  3,  4,  5,  6,  7,

    */

    pub fn matrix_8_8() -> Self {
        Self {
            digits: vec![
                BinaryDigitConfig {
                    off_color: RGB8::new(MIN, 0, 0),
                    on_color: RGB8::new(HIGH, 0, 0),
                    bits: vec![15, 0],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(MIN, 0, 0),
                    on_color: RGB8::new(HIGH, 0, 0),
                    bits: vec![30, 17, 14, 1],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, MIN, 0),
                    on_color: RGB8::new(0, HIGH, 0),
                    bits: vec![19, 12, 3],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, MIN, 0),
                    on_color: RGB8::new(0, HIGH, 0),
                    bits: vec![27, 20, 11, 4],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, MIN),
                    on_color: RGB8::new(0, 0, HIGH),
                    bits: vec![22, 9, 6],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, 0, MIN),
                    on_color: RGB8::new(0, 0, HIGH),
                    bits: vec![24, 23, 8, 7],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, MIN, MIN),
                    on_color: RGB8::new(0, HIGH, HIGH),
                    bits: vec![47, 32],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(0, MIN, MIN),
                    on_color: RGB8::new(0, HIGH, HIGH),
                    bits: vec![62, 49, 46, 33],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(MIN, MIN, 0),
                    on_color: RGB8::new(HIGH, HIGH, 0),
                    bits: vec![51, 44, 35],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(MIN, MIN, 0),
                    on_color: RGB8::new(HIGH, HIGH, 0),
                    bits: vec![59, 52, 43, 36],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(MIN, 0, MIN),
                    on_color: RGB8::new(HIGH, 0, HIGH),
                    bits: vec![54, 41, 38],
                },
                BinaryDigitConfig {
                    off_color: RGB8::new(MIN, 0, MIN),
                    on_color: RGB8::new(HIGH, 0, HIGH),
                    bits: vec![56, 55, 40, 39],
                },
            ],
        }
    }
}

pub struct BinaryClock<T: SmartLeds> {
    task: Option<ClockTask<T>>,
}

impl<'d> BinaryClock<SmartLedsRmt<'d>> {
    pub fn new_rmt(smart_leds: SmartLedsRmt<'d>) -> BinaryClock<SmartLedsRmt<'d>> {
        let x = ClockTask::new_rmt(smart_leds);
        BinaryClock { task: Some(x) }
    }
}

impl<'d, T> BinaryClock<SmartLedsSpi<'d, T>>
where
    T: Borrow<SpiDriver<'d>> + 'd,
{
    pub fn new_spi(smart_leds: SmartLedsSpi<'d, T>) -> BinaryClock<SmartLedsSpi<'d, T>>
    where
        T: Borrow<SpiDriver<'d>> + 'd,
    {
        let x = ClockTask::new_spi(smart_leds);
        BinaryClock { task: Some(x) }
    }
}

impl<T: SmartLeds + 'static> Feature for BinaryClock<T> {
    type Config = BinaryClockConfig;

    fn init(
        &self,
        _initializer: &mut crate::Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        info!("BinaryClock::init()");

        Ok(FeatureDescriptor {
            name: "BinaryClock".to_string(),
            config: BinaryClockConfig::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        _sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        _config: BinaryClockConfig,
    ) -> anyhow::Result<()> {
        match self.task.take() {
            Some(task) => initializer.add_task(Box::new(task), "* * * * * *")?,
            None => anyhow::bail!("BinaryClock task already taken"),
        }

        Ok(())
    }
}

pub struct ClockTask<T: SmartLeds> {
    smart_leds: T,
    config: BinaryClockLayout,
}

impl<'d> ClockTask<SmartLedsRmt<'d>> {
    fn new_rmt(smart_leds: SmartLedsRmt<'d>) -> Self {
        ClockTask {
            smart_leds,
            config: BinaryClockLayout::matrix_8_8(),
        }
    }
}

impl<'d, T> ClockTask<SmartLedsSpi<'d, T>>
where
    T: Borrow<SpiDriver<'d>> + 'd,
{
    fn new_spi(smart_leds: SmartLedsSpi<'d, T>) -> Self {
        ClockTask {
            smart_leds,
            config: BinaryClockLayout::matrix_8_8(),
        }
    }
}

impl<T: SmartLeds> ClockTask<T> {
    fn to_bits(&mut self, digit: usize, v: u32) -> anyhow::Result<()> {
        let digit_config = &self.config.digits[digit];
        let off = digit_config.off_color;
        let on = digit_config.on_color;
        let bits = &digit_config.bits;

        for i in 0..bits.len() {
            let index = bits[i];
            // assert!(index <= self.num_pixels);
            // Extract bit from most-significant to least-significant
            let bit = (v >> (bits.len() - 1 - i)) & 1;

            // self.pixels[index] = if bit == 0 { off } else { on };
            self.smart_leds
                .set_pixel_rgb(index, if bit == 0 { off } else { on })?;
        }
        Ok(())
    }
}

impl<T: SmartLeds> ScheduledTask<Esp32Platform> for ClockTask<T> {
    fn name(&self) -> &str {
        "Binary Clock"
    }

    fn run(&mut self, _sparko_embedded: &mut Esp32Platform) -> anyhow::Result<()> {
        let now = Local::now();

        info!("Current time: {}", now.format("%Y-%m-%d %H:%M:%S"));
        self.to_bits(0, now.hour() / 10)?;
        self.to_bits(1, now.hour() % 10)?;

        self.to_bits(2, now.minute() / 10)?;
        self.to_bits(3, now.minute() % 10)?;

        self.to_bits(4, now.second() / 10)?;
        self.to_bits(5, now.second() % 10)?;

        self.to_bits(6, now.day() / 10)?;
        self.to_bits(7, now.day() % 10)?;

        self.to_bits(8, now.month() / 10)?;
        self.to_bits(9, now.month() % 10)?;

        self.to_bits(10, ((now.year() % 100) / 10) as u32)?;
        self.to_bits(11, (now.year() % 10) as u32)?;

        self.smart_leds.send()?;
        Ok(())
    }
}
