//! The touch screen of the AXS15231B, the Guition JC3248W535C's display and touch controller, on its own I2C bus, which
//! says where the screen is touched, one point at most, when asked with a read command, as ESPHome's axs15231 driver
//! does. The board's touch interrupt pin is not known for certain, so the touch is read every POLL_INTERVAL, and each
//! point, turned into the landscape coordinates the screen is drawn in, see display/axs15231b.rs, is given to the
//! listeners, as AXS5106L's driver does.

use std::sync::Arc;
use std::time::Duration;

use esp_idf_hal::delay::BLOCK;
use esp_idf_hal::i2c::I2cDriver;
use sparko_embedded_std::listener::{Listener, ListenerManager};

use super::TouchPoint;

/// The touch controller's I2C address.
const ADDRESS: u8 = 0x3B;
/// The command which reads the touch, and how many bytes it answers with: a gesture, the number of points, then the
/// point's x and y, 12 bits each, in the panel's own portrait coordinates.
const READ_TOUCH: [u8; 11] = [0xB5, 0xAB, 0xA5, 0x5A, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00];
const ANSWER: usize = 8;
/// How often the touch is read, often enough for a swipe to give several points.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The panel's own size, portrait, see display/axs15231b.rs.
const PANEL_WIDTH: u16 = 320;
const PANEL_HEIGHT: u16 = 480;

pub struct TouchDriver {
    listener_manager: Arc<ListenerManager<TouchPoint>>,
}

impl TouchDriver {
    /// Starts reading the touch on i2c, in a thread of its own, for as long as the device runs.
    pub fn start(mut i2c: I2cDriver<'static>) -> anyhow::Result<TouchDriver> {
        let listener_manager = Arc::new(ListenerManager::new());
        let listeners = listener_manager.clone();
        std::thread::Builder::new().name("touch".into()).stack_size(4096).spawn(move || {
            // Only the first failure is logged, as one which lasts would fill the log.
            let mut failing = false;
            loop {
                std::thread::sleep(POLL_INTERVAL);
                match read(&mut i2c) {
                    Ok(Some(point)) => {
                        failing = false;
                        listeners.emit(&point);
                    }
                    Ok(None) => failing = false,
                    Err(error) => {
                        if !failing {
                            log::warn!("Unable to read the touch screen, {error:#}");
                        }
                        failing = true;
                    }
                }
            }
        })?;
        Ok(TouchDriver { listener_manager })
    }

    pub fn add_listener(&self, listener: &Arc<dyn Listener<TouchPoint>>) {
        self.listener_manager.add_listener(listener);
    }

    pub fn remove_listener(&self, listener: &Arc<dyn Listener<TouchPoint>>) {
        self.listener_manager.remove_listener(listener);
    }
}

/// Where the screen is touched, in the landscape coordinates it is drawn in, None if it is not.
fn read(i2c: &mut I2cDriver<'static>) -> anyhow::Result<Option<TouchPoint>> {
    let mut answer = [0u8; ANSWER];
    i2c.write_read(ADDRESS, &READ_TOUCH, &mut answer, BLOCK)?;
    if answer[0] != 0 || answer[1] == 0 {
        return Ok(None);
    }
    let panel_x = (((answer[2] & 0x0F) as u16) << 8) | answer[3] as u16;
    let panel_y = (((answer[4] & 0x0F) as u16) << 8) | answer[5] as u16;
    if panel_x >= PANEL_WIDTH || panel_y >= PANEL_HEIGHT {
        return Ok(None);
    }
    // As the display turns the frame onto the panel, see its fill_strip, the other way.
    Ok(Some(TouchPoint { x: panel_y, y: PANEL_WIDTH - 1 - panel_x }))
}
