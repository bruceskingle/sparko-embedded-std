

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use esp_idf_hal::gpio::OutputPin;

use esp_idf_hal::gpio::PinDriver;
use sparko_platform::Status;

use crate::led::LedManager;


struct InvertiblePinDriver<'a> {
    pin_driver: PinDriver<'a, esp_idf_hal::gpio::Output>,
    inverted: bool,
}

impl<'a> InvertiblePinDriver<'a> {
    fn new<P: OutputPin + 'a>(pin: P, inverted: bool) -> Self {
        let pin_driver: PinDriver<'a, esp_idf_hal::gpio::Output> = PinDriver::output(pin).unwrap();
        Self {
            pin_driver,
            inverted,
        }
    }

    fn on(&mut self) -> anyhow::Result<()> {
        if self.inverted {
            self.pin_driver.set_low()?;
        } else {
            self.pin_driver.set_high()?;
        }
        Ok(())
    }

    fn off(&mut self) -> anyhow::Result<()> {
        if self.inverted {
            self.pin_driver.set_high()?;
        } else {
            self.pin_driver.set_low()?;
        }
        Ok(())
    }
    
}

#[derive(Clone)]
struct FlashConfig {
    flashes: u32,
    burst: Duration,
    pause: Duration,
}

struct Shared {
    config: FlashConfig,
    updated: bool,
}

pub struct MonoLedManager {
    shared_state: Arc<(Mutex<Shared>, Condvar)>,
}


impl MonoLedManager {
    pub fn new<P: OutputPin + 'static>(
        inverted: bool,
        pin: P,
    ) -> anyhow::Result<Self> {
        let shared_state = Arc::new((Mutex::new(
            Shared {
                config: FlashConfig {
                    flashes: 3,
                    burst: Duration::from_secs(1),
                    pause: Duration::from_secs(1),
                },
                updated: false
            }), Condvar::new()));

        let shared_state_clone = shared_state.clone();

        let result = Self {
            shared_state,
         };

        std::thread::spawn(move || {
            let mut pin_driver = InvertiblePinDriver::new(pin, inverted);

            let (lock , cond_var) = &*shared_state_clone;

            loop {

                let mut state = lock.lock().unwrap();
                let config = state.config.clone();
                state.updated = false;
                drop(state);
                
                if config.flashes == 0 {
                    Self::wait_or_interrupt(lock, cond_var, Duration::from_secs(300));
                }
                else if config.flashes == 1 && config.burst == Duration::from_secs(0) && config.pause == Duration::from_secs(0) {
                    pin_driver.on().unwrap();
                    Self::wait_or_interrupt(lock, cond_var, Duration::from_secs(300));
                }
                else {
                    Self::flash(&mut pin_driver, lock, cond_var, config);
                }
            }
        });
        
        Ok(result)
    }

    fn flash(pin_driver: &mut InvertiblePinDriver, lock: &Mutex<Shared>, cond_var: &Condvar, config: FlashConfig) {
        let on_off = config.burst / (config.flashes * 2);

        for _ in 0..config.flashes {
            pin_driver.on().unwrap();
            if Self::wait_or_interrupt(lock, cond_var, on_off) {
                return;
            }

            pin_driver.off().unwrap();
            if Self::wait_or_interrupt(lock, cond_var, on_off) {
                return;
            }
        }
        
        Self::wait_or_interrupt(lock, cond_var, config.pause);
    }

    fn wait_or_interrupt(
        lock: &Mutex<Shared>,
        cvar: &Condvar,
        timeout: Duration,
    ) -> bool {
        let state = lock.lock().unwrap();

        let (state, _) = cvar
            .wait_timeout(state, timeout)
            .unwrap();

        state.updated
    }

    pub fn set_flash_config(&self, flashes: u32, burst: Duration, pause: Duration) -> anyhow::Result<()> {
        if burst < Duration::from_millis(100) || pause < Duration::from_millis(100) {
            anyhow::bail!("Burst and pause durations must be at least 100ms");
        }

        let (lock , cond_var) = &*self.shared_state;
        let mut shared_state = lock.lock().unwrap();
        
        shared_state.config.flashes = flashes;
        shared_state.config.burst = burst;
        shared_state.config.pause = pause;
        
        shared_state.updated = true;

        cond_var.notify_all();
        Ok(())
    }

    pub fn set_flashes(&self, flashes: u32) {
        let (lock , cond_var) = &*self.shared_state;
        let mut shared_state = lock.lock().unwrap();

        shared_state.config.flashes = flashes;
        
        shared_state.updated = true;

        cond_var.notify_all();
    }
}


impl LedManager for MonoLedManager {
    
    fn set_on(&self) -> anyhow::Result<()> {
        let (lock , cond_var) = &*self.shared_state;
        let mut shared_state = lock.lock().unwrap();

        shared_state.config.flashes = 1;
        shared_state.config.burst = Duration::from_secs(0);
        shared_state.config.pause = Duration::from_secs(0);

        shared_state.updated = true;

        cond_var.notify_all();
        Ok(())
    }

    fn set_off(&self) -> anyhow::Result<()> {
        let (lock , cond_var) = &*self.shared_state;
        let mut shared_state = lock.lock().unwrap();

        shared_state.config.flashes = 0;

        shared_state.updated = true;

        cond_var.notify_all();
        Ok(())
    }

    fn set_status(&self, status: &Status) -> anyhow::Result<()> {
        match status {
            Status::Initializing(_) => self.set_flashes(1),
            Status::Running => self.set_off()?,
            Status::Setup => self.set_flashes(2),
            Status::Error => self.set_flashes(3),
        };
        Ok(())
    }
}
