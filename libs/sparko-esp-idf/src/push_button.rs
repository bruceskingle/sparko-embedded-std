//! A push button on a GPIO pin, which runs its action, if it has one, each time it is pressed, e.g. a board's KEY
//! button, which the platform owns with the key-button feature, see Esp32Platform::key_button, or a button an app wires
//! to a pin of the Remainder. The button is taken to connect the pin to ground when pressed, with the chip's own
//! pull-up, as board buttons usually do.
//!
//! The action can be set, or replaced, at any time, by the app, see Esp32PlatformBuilder::with_key_button, or by a
//! feature when it starts. It runs on the button's own thread, so it may take its time, e.g. to show a message for a
//! while. Presses while it runs are not counted.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use esp_idf_hal::gpio::{InputPin, PinDriver, Pull};
use log::info;

/// How often the button is looked at, short enough to catch a quick press.
const POLL: Duration = Duration::from_millis(20);

/// What a press does, see PushButton::set_action.
pub type Action = Box<dyn FnMut() -> anyhow::Result<()> + Send>;

/// A handle on the button, which can be cloned, e.g. for a feature to set its action.
#[derive(Clone)]
pub struct PushButton {
    name: &'static str,
    action: Arc<Mutex<Option<Action>>>,
}

impl PushButton {
    /// The button on pin, called name in the log, e.g. KEY, watched from now on, with no action until one is set.
    /// Holding it down is one press.
    pub fn new(name: &'static str, pin: impl InputPin + 'static) -> anyhow::Result<Self> {
        let button = PinDriver::input(pin, Pull::Up)?;
        let action: Arc<Mutex<Option<Action>>> = Arc::new(Mutex::new(None));
        let pressed_action = action.clone();
        std::thread::Builder::new().name(format!("{name} button")).stack_size(4096).spawn(move || {
            let mut was_pressed = false;
            loop {
                std::thread::sleep(POLL);
                let pressed = button.is_low();
                if pressed && !was_pressed {
                    info!("{name} pressed");
                    if let Some(action) = pressed_action.lock().unwrap().as_mut() {
                        if let Err(error) = action() {
                            log::warn!("The action of {name} failed, {error:#}");
                        }
                    }
                }
                was_pressed = pressed;
            }
        })?;
        Ok(PushButton { name, action })
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Makes each press run action, in place of any action before.
    pub fn set_action(&self, action: Action) {
        *self.action.lock().unwrap() = Some(action);
    }

    /// Makes presses do nothing.
    pub fn clear_action(&self) {
        *self.action.lock().unwrap() = None;
    }
}
