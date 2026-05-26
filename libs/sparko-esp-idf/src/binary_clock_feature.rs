use std::borrow::Borrow;
use std::net::IpAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::Datelike;
use chrono::Local;
use chrono::Timelike;

use embedded_graphics::prelude::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;
use esp_idf_hal::gpio::OutputPin;
use esp_idf_hal::spi::SpiDeviceDriver;
use esp_idf_hal::spi::SpiDriver;
use esp_idf_svc::http::Method;
use esp_idf_svc::http::client::EspHttpConnection;
use log::info;
use rgb::RGB8;
use sparko_embedded_std::Layout;
use sparko_embedded_std::config::Config;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config::ConfigSpecValue;
use sparko_embedded_std::config::TypedValue;
use sparko_embedded_std::graphics::ClockRenderer;
use sparko_embedded_std::graphics::DisplayManager;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;

use esp_idf_svc::hal::rmt::RmtChannel;
// use smart_leds::{RGB8, SmartLedsWrite, hsv::{Hsv, hsv2rgb}};
// use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use crate::esp32_platform::Esp32Platform;
use crate::esp32_platform::Esp32PlatformInitializer;
use crate::smart_led::SmartLeds;
use crate::smart_led::SmartLedsRmt;
use crate::smart_led::SmartLedsSpi;
use crate::{Feature, FeatureDescriptor};

//                                           123456789012345<-------- Max Name Length 15
// pub const USER_NAME: &str =                 "user_name";
// pub const PASSWORD: &str =                  "password";
// pub const HOSTNAME: &str =                  "hostname";
// pub const BASE_SERVICE_URL: &str =          "base_url";
// pub const GET_IP_URL: &str =                "get_ip_url";
// pub const GET_REQUIRES_STRIP: &str =        "get_req_strip";
// pub const UPDATE_URL: &str =                "update_url";
// pub const UPDATE_REQUIRES_ADDRESS: &str =   "upd_req_addr";
// pub const UPDATE_INTERVAL: &str =           "upd_int";
// pub const SCHEDULE: &str =                  "schedule";

const HIGH: u8 = 16;
const LOW: u8 = 4;
const MIN: u8 = 0;

pub struct BinaryDigitConfig {
    off_colour: rgb::RGB<u8>,
    on_colour: rgb::RGB<u8>,
    bits: Vec<usize>,
}

pub struct BinaryClockConfig {
    // off_colour: smart_leds::RGB<u8>,
    // h1_colour: smart_leds::RGB<u8>,
    // h2_colour: smart_leds::RGB<u8>,
    // m1_colour: smart_leds::RGB<u8>,
    // m2_colour: smart_leds::RGB<u8>,
    // s1_colour: smart_leds::RGB<u8>,
    // s2_colour: smart_leds::RGB<u8>,
    num_pixels: usize,
    digits: Vec<BinaryDigitConfig>,
    // h1_pixels: [usize; 2],
    // h2_pixels: [usize; 4],
    // m1_pixels: [usize; 3],
    // m2_pixels: [usize; 4],
    // s1_pixels: [usize; 3],
    // s2_pixels: [usize; 4],
}

impl BinaryClockConfig {
    pub fn default() -> Self {
        Self {
            num_pixels: 20,
            digits: vec![
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, 0),
                    on_colour: RGB8::new(HIGH, 0, 0),
                    bits: vec![0, 1],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, 0),
                    on_colour: RGB8::new(HIGH, LOW, LOW),
                    bits: vec![2, 3, 4, 5],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, 0),
                    on_colour: RGB8::new(0, HIGH, 0),
                    bits: vec![6, 7, 8],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, 0),
                    on_colour: RGB8::new(LOW, HIGH, LOW),
                    bits: vec![9, 10, 11, 12],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, 0),
                    on_colour: RGB8::new(0, 0, HIGH),
                    bits: vec![13, 14, 15],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, 0),
                    on_colour: RGB8::new(LOW, LOW, HIGH),
                    bits: vec![16, 17, 18, 19],
                },
            ],
            // off_colour: RGB8::new(0, 0, 0),
            // h1_colour: RGB8::new(HIGH, 0, 0),
            // h2_colour: RGB8::new(HIGH, LOW, LOW),
            // m1_colour: RGB8::new(0, HIGH, 0),
            // m2_colour: RGB8::new(LOW, HIGH, LOW),
            // s1_colour: RGB8::new(0, 0, HIGH),
            // s2_colour: RGB8::new(LOW, LOW, HIGH),

            // h1_pixels: [0, 1],
            // h2_pixels: [2, 3, 4, 5],
            // m1_pixels: [6, 7, 8],
            // m2_pixels: [9, 10, 11, 12],
            // s1_pixels: [13, 14, 15],
            // s2_pixels: [16, 17, 18, 19],
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
            num_pixels: 64,
            digits: vec![
                BinaryDigitConfig {
                    off_colour: RGB8::new(MIN, 0, 0),
                    on_colour: RGB8::new(HIGH, 0, 0),
                    bits: vec![15, 0],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(MIN, 0, 0),
                    on_colour: RGB8::new(HIGH, 0, 0),
                    bits: vec![30, 17, 14, 1],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, MIN, 0),
                    on_colour: RGB8::new(0, HIGH, 0),
                    bits: vec![19, 12, 3],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, MIN, 0),
                    on_colour: RGB8::new(0, HIGH, 0),
                    bits: vec![27, 20, 11, 4],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, MIN),
                    on_colour: RGB8::new(0, 0, HIGH),
                    bits: vec![22, 9, 6],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, 0, MIN),
                    on_colour: RGB8::new(0, 0, HIGH),
                    bits: vec![24, 23, 8, 7],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, MIN, MIN),
                    on_colour: RGB8::new(0, HIGH, HIGH),
                    bits: vec![47, 32],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(0, MIN, MIN),
                    on_colour: RGB8::new(0, HIGH, HIGH),
                    bits: vec![62, 49, 46, 33],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(MIN, MIN, 0),
                    on_colour: RGB8::new(HIGH, HIGH, 0),
                    bits: vec![51, 44, 35],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(MIN, MIN, 0),
                    on_colour: RGB8::new(HIGH, HIGH, 0),
                    bits: vec![59, 52, 43, 36],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(MIN, 0, MIN),
                    on_colour: RGB8::new(HIGH, 0, HIGH),
                    bits: vec![54, 41, 38],
                },
                BinaryDigitConfig {
                    off_colour: RGB8::new(MIN, 0, MIN),
                    on_colour: RGB8::new(HIGH, 0, HIGH),
                    bits: vec![56, 55, 40, 39],
                },
            ],
            // off_colour: RGB8::new(0, 0, 0),
            // h1_colour: RGB8::new(HIGH, 0, 0),
            // h2_colour: RGB8::new(HIGH, LOW, LOW),
            // m1_colour: RGB8::new(0, HIGH, 0),
            // m2_colour: RGB8::new(LOW, HIGH, LOW),
            // s1_colour: RGB8::new(0, 0, HIGH),
            // s2_colour: RGB8::new(LOW, LOW, HIGH),
            // h1_pixels: [0, 1],
            // h2_pixels: [15, 14, 13, 12],
            // m1_pixels: [16, 17, 18],
            // m2_pixels: [31, 30, 29, 28],
            // s1_pixels: [32, 33, 34],
            // s2_pixels: [47, 46, 45, 44],
        }
    }
}

pub struct BinaryClockFeature<T: SmartLeds> {
    task: Option<ClockTask<T>>,
}

// pub fn required_spi_transfer_size() -> usize {
//     crate::smart_led::required_spi_transfer_size(64) // WARNING: hardcoded for 64 LEDs, should be configurable
// }

impl<'d> BinaryClockFeature<SmartLedsRmt<'d>> {
    pub fn new_rmt(smart_leds: SmartLedsRmt<'d>) -> BinaryClockFeature<SmartLedsRmt<'d>> {
        let x = ClockTask::new_rmt(smart_leds);
        BinaryClockFeature { task: Some(x) }
    }
}

// impl<'d, T> BinaryClockFeature<'d, T>
// where
//     T: Borrow<SpiDriver<'d>> + 'd,

// impl <SmartLedsSpi<'static>>
impl<'d, T> BinaryClockFeature<SmartLedsSpi<'d, T>>
where
    T: Borrow<SpiDriver<'d>> + 'd,
{
    pub fn new_spi(smart_leds: SmartLedsSpi<'d, T>) -> BinaryClockFeature<SmartLedsSpi<'d, T>>
    where
        T: Borrow<SpiDriver<'d>> + 'd,
    {
        let x = ClockTask::new_spi(smart_leds);
        BinaryClockFeature { task: Some(x) }
    }
}

impl<T: SmartLeds + 'static> Feature for BinaryClockFeature<T> {
    fn init(
        &self,
        _initializer: &mut crate::esp32_platform::Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        info!("BinaryClock::init()");
        let config = ConfigSpec::builder()
            // .with(USER_NAME.to_string(), ConfigSpecValue::new(TypedValue::String(32, None), true))?
            // .with(PASSWORD.to_string(), ConfigSpecValue::new(TypedValue::String(32, None), true))?
            // .with(HOSTNAME.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // // .with(BASE_SERVICE_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // .with(GET_IP_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // // .with(GET_REQUIRES_STRIP.to_string(), ConfigSpecValue::new(TypedValue::Bool(false), false))?
            // .with(UPDATE_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // .with(UPDATE_REQUIRES_ADDRESS.to_string(), ConfigSpecValue::new(TypedValue::Bool(false), false ))?
            // .with(SCHEDULE.to_string(), ConfigSpecValue::new(TypedValue::Cron(None), true))?
            .build();

        Ok(FeatureDescriptor {
            name: "BinaryClock".to_string(),
            config,
        })
    }

    fn start(
        &mut self,
        sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &Config,
    ) -> anyhow::Result<()> {
        match self.task.take() {
            Some(task) => initializer.add_task(Box::new(task), "* * * * * *")?,
            None => anyhow::bail!("BinaryClock task already taken"),
        }

        Ok(())
    }
}

pub struct ClockTask<T: SmartLeds> {
    // ws2812: ws2812_esp32_rmt_driver::LedPixelEsp32Rmt<'static,
    //     smart_leds::RGB<u8>,
    //     ws2812_esp32_rmt_driver::driver::color::LedPixelColorImpl<3, 1, 0, 2, i>>,
    smart_leds: T,
    config: BinaryClockConfig,
    i: usize,
}

impl<'d> ClockTask<SmartLedsRmt<'d>> {
    fn new_rmt(smart_leds: SmartLedsRmt<'d>) -> Self {
        ClockTask {
            smart_leds,
            config: BinaryClockConfig::matrix_8_8(),
            i: 0,
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
            config: BinaryClockConfig::matrix_8_8(),
            i: 0,
        }
    }
}

impl<T: SmartLeds> ClockTask<T> {
    fn to_bits(&mut self, digit: usize, v: u32) -> anyhow::Result<()> {
        let digit_config = &self.config.digits[digit];
        let off = digit_config.off_colour;
        let on = digit_config.on_colour;
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
    // fn run(&mut self, _sparko_cyd: &dyn SparkoEmbeddedStd) -> anyhow::Result<()> {
    //     let clock_renderer =
    // }

    fn name(&self) -> &str {
        "Binary Clock"
    }

    fn run(&mut self, sparko_embedded: &mut Esp32Platform) -> anyhow::Result<()> {
        // let mut i: u8 = 0;
        // for j in 0..self.i {
        //     self.smart_leds.set_pixel_rgb(j, RGB8 {r: 0, g: 0, b: 0})?;
        //     i += 1;
        // }
        // let idx = i as usize;
        // self.smart_leds.set_pixel_rgb(idx, RGB8 {r: i, g: 0, b: 0})?;
        // self.smart_leds.set_pixel_rgb(idx + 1, RGB8 {r: 0, g: i, b: 0})?;
        // self.smart_leds.set_pixel_rgb(idx + 2, RGB8 {r: 0, g: 0, b: i})?;
        // self.smart_leds.set_pixel_rgb(idx + 3, RGB8 {r: i, g: i, b: 0})?;
        // self.smart_leds.set_pixel_rgb(idx + 4, RGB8 {r: i, g: 0, b: i})?;
        // self.smart_leds.set_pixel_rgb(idx + 5, RGB8 {r: 0, g: i, b: i})?;

        // self.i += 1;

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
