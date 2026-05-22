use esp_idf_hal::gpio::OutputPin;
use esp_idf_hal::ledc::{LedcChannel, LedcTimer, LowSpeed};
use std::sync::{Arc, Mutex};

use esp_idf_hal::ledc::LedcDriver;
use esp_idf_hal::units::*;

use sparko_platform::Status;

use crate::led::LedManager;

pub struct RgbLedManager<'a> {
    inverted: bool,
    brightness: u8,
    led_timer_driver: esp_idf_hal::ledc::LedcTimerDriver<'a, esp_idf_hal::ledc::LowSpeed>,
    led_channel_red: Arc<Mutex<esp_idf_hal::ledc::LedcDriver<'a>>>,
    led_channel_green: Arc<Mutex<esp_idf_hal::ledc::LedcDriver<'a>>>,
    led_channel_blue: Arc<Mutex<esp_idf_hal::ledc::LedcDriver<'a>>>,
}

impl<'a> RgbLedManager<'a> {
    pub fn new<
        T: LedcTimer<SpeedMode = LowSpeed> + 'a,
        CR: LedcChannel<SpeedMode = LowSpeed> + 'a,
        PR: OutputPin + 'a,
        CG: LedcChannel<SpeedMode = LowSpeed> + 'a,
        PG: OutputPin + 'a,
        CB: LedcChannel<SpeedMode = LowSpeed> + 'a,
        PB: OutputPin + 'a,
    >(
        inverted: bool,
        brightness: u8,
        timer0: T,
        red_channel: CR,
        red_pin: PR,
        green_channel: CG,
        green_pin: PG,
        blue_channel: CB,
        blue_pin: PB,
    ) -> anyhow::Result<Self> {
        let led_timer_driver = esp_idf_hal::ledc::LedcTimerDriver::new(
            timer0,
            &esp_idf_hal::ledc::config::TimerConfig::new().frequency(1000.Hz()),
        )?;

        let led_channel_red = Arc::new(Mutex::new(LedcDriver::new(
            red_channel,
            &led_timer_driver,
            red_pin,
        )?));
        let led_channel_green = Arc::new(Mutex::new(LedcDriver::new(
            green_channel,
            &led_timer_driver,
            green_pin,
        )?));
        let led_channel_blue = Arc::new(Mutex::new(LedcDriver::new(
            blue_channel,
            &led_timer_driver,
            blue_pin,
        )?));

        Ok(Self {
            inverted,
            brightness,
            led_timer_driver,
            led_channel_red,
            led_channel_green,
            led_channel_blue,
        })
    }

    fn apply_inversion(&self, value: u8) -> u8 {
        let value = (value as u16 * self.brightness as u16 / 255) as u8;
        if self.inverted { 255 - value } else { value }
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8) -> anyhow::Result<()> {
        self.led_channel_red
            .lock()
            .unwrap()
            .set_duty(self.apply_inversion(r) as u32)?;
        self.led_channel_green
            .lock()
            .unwrap()
            .set_duty(self.apply_inversion(g) as u32)?;
        self.led_channel_blue
            .lock()
            .unwrap()
            .set_duty(self.apply_inversion(b) as u32)?;
        Ok(())
    }
}

impl LedManager for RgbLedManager<'_> {
    fn set_on(&self) -> anyhow::Result<()> {
        self.set_color(255, 255, 255)?;
        Ok(())
    }

    fn set_off(&self) -> anyhow::Result<()> {
        self.set_color(0, 0, 0)?;
        Ok(())
    }

    fn set_status(&self, status: &Status) -> anyhow::Result<()> {
        match status {
            Status::Initializing(_) => self.set_color(255, 255, 0)?,
            Status::Running => self.set_color(0, 255, 0)?,
            Status::Setup => self.set_color(0, 0, 255)?,
            Status::Error => self.set_color(255, 0, 0)?,
        };
        Ok(())
    }
}
