use esp_idf_hal::gpio::OutputPin;
use esp_idf_hal::gpio::PinDriver;
use std::sync::{Arc, Mutex};

use sparko_embedded_std::Status;

use crate::led::LedManager;

pub struct SimpleLedManager<'d> {
    inverted: bool,
    led: Arc<Mutex<PinDriver<'d, esp_idf_hal::gpio::Output>>>,
}

impl<'d> SimpleLedManager<'d> {
    pub fn new<T: OutputPin + 'd>(inverted: bool, pin: T) -> Self {
        let led = Arc::new(Mutex::new(PinDriver::output(pin).unwrap()));
        Self { inverted, led }
    }
}

impl LedManager for SimpleLedManager<'_> {
    fn set_on(&self) -> anyhow::Result<()> {
        if self.inverted {
            self.led.lock().unwrap().set_low()?;
        } else {
            self.led.lock().unwrap().set_high()?;
        }
        Ok(())
    }

    fn set_off(&self) -> anyhow::Result<()> {
        if self.inverted {
            self.led.lock().unwrap().set_high()?;
        } else {
            self.led.lock().unwrap().set_low()?;
        }
        Ok(())
    }

    fn set_status(&self, status: &Status) -> anyhow::Result<()> {
        match status {
            Status::Initializing(_) => self.set_on()?,
            Status::Running => self.set_off()?,
            Status::Setup => self.set_on()?,
            Status::Error => self.set_on()?,
        };
        Ok(())
    }
}
