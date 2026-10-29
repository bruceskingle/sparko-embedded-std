//! Adoption: an app, e.g. Marco Sparko, takes over managing the device, after which only it can use the API, see
//! api.rs, and the /config page only says so, see ConfigManager::set_managed.
//!
//! 1. The app asks the device to start pairing, and the device shows a 6 digit code on its screen, see
//!    AdoptionOverlay, with the start of its certificate's fingerprint, which the app also shows, for the careful.
//! 2. The user types the code into the app, which sends it with a key it made, 32 random bytes as hex. If the code
//!    is right, the device keeps the key, and from then on every API request but /api/info must have it, as
//!    Authorization: Bearer <key>, which HTTPS keeps secret.
//!
//! A code lasts PAIRING_TIME and MAX_ATTEMPTS wrong guesses, after which pairing must start again, which needs someone
//! to read the screen. The app releases the device through the API when it no longer manages it, or if the app has
//! lost the key, holding the BOOT button for RELEASE_HOLD does, see watch_boot_button.
//!
//! This trusts the network while pairing: someone intercepting the traffic then could adopt the device instead.
//! Adoption over USB, by the app which flashes the device, will not.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use log::{info, warn};
use sparko_embedded_std::config_manager::ConfigManager;

use crate::identity::DEVICE_NAMESPACE;

const KEY_KEY: &str = "api_key";
pub const PAIRING_TIME: Duration = Duration::from_secs(120);
const MAX_ATTEMPTS: u8 = 3;
/// How long the BOOT button must be held to release the device.
const RELEASE_HOLD: Duration = Duration::from_secs(5);
/// How long the screen says the device has been adopted or released, see AdoptionOverlay.
const NOTICE_TIME: Duration = Duration::from_secs(5);

struct Pairing {
    code: String,
    expires: Instant,
    attempts: u8,
}

struct State {
    /// The adopting app's key, as 64 lower case hex digits, None if the device has not been adopted.
    key: Option<String>,
    pairing: Option<Pairing>,
    /// What the screen says until when, e.g. that the device has been adopted, see AdoptionOverlay.
    notice: Option<(&'static str, Instant)>,
}

pub struct Adoption {
    nvs: Mutex<EspNvs<NvsDefault>>,
    state: Mutex<State>,
    config_manager: Arc<ConfigManager>,
}

impl Adoption {
    pub fn load(nvs_partition: &EspNvsPartition<NvsDefault>, config_manager: Arc<ConfigManager>) -> anyhow::Result<Arc<Adoption>> {
        let nvs = EspNvs::new(nvs_partition.clone(), DEVICE_NAMESPACE, true)?;
        let mut buffer = [0u8; 80];
        let key = nvs.get_str(KEY_KEY, &mut buffer)?.map(String::from);
        info!("The device is {}", if key.is_some() { "adopted" } else { "not adopted" });
        config_manager.set_managed(key.is_some());
        Ok(Arc::new(Adoption { nvs: Mutex::new(nvs), state: Mutex::new(State { key, pairing: None, notice: None }), config_manager }))
    }

    pub fn is_adopted(&self) -> bool {
        self.state.lock().unwrap().key.is_some()
    }

    /// Whether a request with this Authorization header may use the API: any if the device has not been adopted,
    /// otherwise only one with the key.
    pub fn is_authorised(&self, authorization: Option<&str>) -> bool {
        let state = self.state.lock().unwrap();
        let Some(key) = &state.key else {
            return true;
        };
        let given = authorization.and_then(|header| header.strip_prefix("Bearer ")).unwrap_or_default();
        // In constant time, so that the time taken does not tell how much of it was right.
        given.len() == key.len() && given.bytes().zip(key.bytes()).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
    }

    /// Starts pairing with a new code, replacing any before it, returning how long it lasts.
    pub fn start_pairing(&self) -> anyhow::Result<Duration> {
        let mut state = self.state.lock().unwrap();
        if state.key.is_some() {
            bail!("the device has already been adopted, release it first");
        }
        let mut random = [0u8; 4];
        unsafe { esp_idf_sys::esp_fill_random(random.as_mut_ptr() as *mut _, random.len()) };
        let code = format!("{:06}", u32::from_le_bytes(random) % 1_000_000);
        state.pairing = Some(Pairing { code, expires: Instant::now() + PAIRING_TIME, attempts: 0 });
        info!("Pairing started");
        Ok(PAIRING_TIME)
    }

    /// What the screen should say, if anything, see AdoptionOverlay.
    pub fn notice(&self) -> Option<&'static str> {
        let mut state = self.state.lock().unwrap();
        if state.notice.is_some_and(|(_, until)| until <= Instant::now()) {
            state.notice = None;
        }
        state.notice.map(|(text, _)| text)
    }

    /// The code being shown and how long it has left, if pairing, see AdoptionOverlay.
    pub fn pairing(&self) -> Option<(String, Duration)> {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        if state.pairing.as_ref().is_some_and(|pairing| pairing.expires <= now) {
            info!("Pairing timed out");
            state.pairing = None;
        }
        state.pairing.as_ref().map(|pairing| (pairing.code.clone(), pairing.expires - now))
    }

    /// Adopts the device for the app with key, if code is the one being shown.
    pub fn adopt(&self, code: &str, key: &str) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.key.is_some() {
            bail!("the device has already been adopted");
        }
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("the key must be 32 bytes as 64 hex digits");
        }
        let pairing = state
            .pairing
            .as_mut()
            .filter(|pairing| pairing.expires > Instant::now())
            .ok_or_else(|| anyhow!("the device is not pairing, start again"))?;
        if pairing.code != code.trim().replace(' ', "") {
            pairing.attempts += 1;
            warn!("Wrong pairing code, attempt {}", pairing.attempts);
            if pairing.attempts >= MAX_ATTEMPTS {
                state.pairing = None;
                bail!("the code is wrong, and that was the last try, start again");
            }
            bail!("the code is wrong");
        }
        let key = key.to_ascii_lowercase();
        self.nvs.lock().unwrap().set_str(KEY_KEY, &key)?;
        state.key = Some(key);
        state.pairing = None;
        state.notice = Some(("Adopted", Instant::now() + NOTICE_TIME));
        self.config_manager.set_managed(true);
        info!("The device has been adopted");
        Ok(())
    }

    /// Forgets the adopting app's key, so that the device can be managed by its /config page, or adopted again.
    pub fn release(&self) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        self.nvs.lock().unwrap().remove(KEY_KEY)?;
        state.key = None;
        state.pairing = None;
        state.notice = Some(("Released", Instant::now() + NOTICE_TIME));
        self.config_manager.set_managed(false);
        info!("The device has been released");
        Ok(())
    }
}

/// Releases the device when the BOOT button is held down for RELEASE_HOLD, in a thread of its own, so that it works
/// whatever else the device is doing, in AP mode too.
pub fn watch_boot_button(adoption: Arc<Adoption>, pin: impl esp_idf_hal::gpio::InputPin + 'static) -> anyhow::Result<()> {
    let button = esp_idf_hal::gpio::PinDriver::input(pin, esp_idf_hal::gpio::Pull::Up)?;
    std::thread::Builder::new().stack_size(4096).spawn(move || {
        let mut pressed_since: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if button.is_low() {
                let since = *pressed_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= RELEASE_HOLD && adoption.is_adopted() {
                    info!("BOOT held for {} seconds, releasing the device", RELEASE_HOLD.as_secs());
                    if let Err(error) = adoption.release() {
                        log::error!("Unable to release the device, {error:#}");
                    }
                }
            } else {
                pressed_since = None;
            }
        }
    })?;
    Ok(())
}

/// What AdoptionOverlay is showing.
#[cfg(feature = "display")]
#[derive(PartialEq)]
enum Shown {
    /// The code and the seconds left.
    Pairing(String, u64),
    Notice(&'static str),
}

/// Shows the pairing code on the whole screen while pairing, then for a few seconds that the device has been adopted or
/// released, see DisplayManager::set_overlay.
#[cfg(feature = "display")]
pub struct AdoptionOverlay<DM: sparko_embedded_std::graphics::DisplayManager> {
    display_manager: Arc<Mutex<DM>>,
    adoption: Arc<Adoption>,
    fingerprint: String,
    /// What was last shown, so that it is only drawn when that changes.
    shown: Option<Shown>,
}

#[cfg(feature = "display")]
impl<DM: sparko_embedded_std::graphics::DisplayManager> AdoptionOverlay<DM> {
    pub fn new(display_manager: &Arc<Mutex<DM>>, adoption: Arc<Adoption>, fingerprint: &str) -> Self {
        Self { display_manager: display_manager.clone(), adoption, fingerprint: fingerprint.to_string(), shown: None }
    }

    /// Draws the whole overlay if all is true, otherwise only the countdown, over a black rectangle, so that the
    /// rest does not flash each second.
    fn draw(&self, code: &str, seconds: u64, all: bool) -> anyhow::Result<()> {
        use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_10X20};
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
        use embedded_graphics::text::{Alignment, Text};
        use sparko_embedded_std::graphics::{Color, DrawTargetResultExt};

        /// How far below the centre the countdown's baseline is.
        const COUNTDOWN_Y: i32 = 70;

        let mut manager = self.display_manager.lock().unwrap();
        if !manager.overlay_active() {
            manager.set_overlay(true)?;
        }
        if all {
            manager.fill_color(Color::Black)?;
        }
        let black = manager.map_color(&Color::Black);
        let white = MonoTextStyle::new(&FONT_10X20, manager.map_color(&Color::White));
        let yellow = MonoTextStyle::new(&FONT_10X20, manager.map_color(&Color::Yellow));
        let display = manager.display();
        let bounds = display.bounding_box();
        let centre = bounds.center();
        if all {
            let lines = [
                ("Pairing code", white, -60),
                (&format!("{} {}", &code[..3], &code[3..]), yellow, -20),
                (&format!("Certificate {}", &self.fingerprint[..8]), white, 30),
            ];
            for (text, style, dy) in lines {
                Text::with_alignment(text, centre + Point::new(0, dy), style, Alignment::Center).draw(display).anyhow()?;
            }
        } else {
            // FONT_10X20 reaches 16 above its baseline and 4 below.
            Rectangle::new(Point::new(bounds.top_left.x, centre.y + COUNTDOWN_Y - 18), Size::new(bounds.size.width, 24))
                .into_styled(PrimitiveStyle::with_fill(black))
                .draw(display)
                .anyhow()?;
        }
        Text::with_alignment(&format!("{seconds} s"), centre + Point::new(0, COUNTDOWN_Y), white, Alignment::Center)
            .draw(display)
            .anyhow()?;
        Ok(())
    }

    /// Draws text alone in the middle of the screen, e.g. Adopted.
    fn draw_notice(&self, text: &str) -> anyhow::Result<()> {
        use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_10X20};
        use embedded_graphics::prelude::*;
        use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
        use sparko_embedded_std::graphics::{Color, DrawTargetResultExt};

        let mut manager = self.display_manager.lock().unwrap();
        if !manager.overlay_active() {
            manager.set_overlay(true)?;
        }
        manager.fill_color(Color::Black)?;
        let yellow = MonoTextStyle::new(&FONT_10X20, manager.map_color(&Color::Yellow));
        let display = manager.display();
        let centre = display.bounding_box().center();
        let style = TextStyleBuilder::new().alignment(Alignment::Center).baseline(Baseline::Middle).build();
        Text::with_text_style(text, centre, yellow, style).draw(display).anyhow()?;
        Ok(())
    }
}

#[cfg(feature = "display")]
impl<DM: sparko_embedded_std::graphics::DisplayManager> sparko_embedded_std::task::scheduler::ScheduledTask<crate::Esp32Platform>
    for AdoptionOverlay<DM>
{
    fn run(&mut self, _sparko: &mut crate::Esp32Platform) -> anyhow::Result<()> {
        let shown = match (self.adoption.pairing(), self.adoption.notice()) {
            (Some((code, left)), _) => Some(Shown::Pairing(code, left.as_secs())),
            (None, Some(text)) => Some(Shown::Notice(text)),
            (None, None) => None,
        };
        if shown == self.shown {
            return Ok(());
        }
        match &shown {
            Some(Shown::Pairing(code, seconds)) => {
                // Only the countdown, unless the code is new.
                let all = !matches!(&self.shown, Some(Shown::Pairing(shown_code, _)) if shown_code == code);
                self.draw(code, *seconds, all)?;
            }
            Some(Shown::Notice(text)) => self.draw_notice(text)?,
            None => self.display_manager.lock().unwrap().set_overlay(false)?,
        }
        self.shown = shown;
        Ok(())
    }

    fn name(&self) -> &str {
        "Adoption Overlay"
    }
}
