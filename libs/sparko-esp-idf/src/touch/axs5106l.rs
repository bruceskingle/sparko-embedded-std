use esp_idf_hal::delay::{BLOCK, FreeRtos};
use esp_idf_hal::gpio::{InputPin, OutputPin, InterruptType, PinDriver, Pull};
use esp_idf_hal::i2c::I2cDriver;
use esp_idf_hal::task::notification::Notification;
use sparko_embedded_std::DisplayOrientation;
use sparko_embedded_std::listener::{Listener, ListenerManager};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
}

pub struct TouchData {
    pub points: Vec<TouchPoint>,
}

pub struct TouchDriverFactory {
    int_pin: Arc<Mutex<PinDriver<'static, esp_idf_hal::gpio::Input>>>,
    rst_pin: PinDriver<'static, esp_idf_hal::gpio::Output>,
    i2c: Arc<Mutex<I2cDriver<'static>>>,
}

impl TouchDriverFactory {
    pub fn build(
        self,
        width: u16,
        height: u16,
        rotation: DisplayOrientation,
    ) -> anyhow::Result<TouchDriver> {
        Ok(TouchDriver{
            int_pin: self.int_pin,
            rst_pin: self.rst_pin,
            i2c: self.i2c,
            width,
            height,
            rotation,
            listener_manager: Arc::new(ListenerManager::new()),
        })
    }
}

pub struct TouchDriver {
    int_pin: Arc<Mutex<PinDriver<'static, esp_idf_hal::gpio::Input>>>,
    rst_pin: PinDriver<'static, esp_idf_hal::gpio::Output>,
    i2c: Arc<Mutex<I2cDriver<'static>>>,
    width: u16,
    height: u16,
    rotation: DisplayOrientation,
    listener_manager: Arc<ListenerManager<TouchPoint>>,
}

impl TouchDriver {
    pub fn factory(
        int_pin: impl InputPin + 'static,
        rst_pin: impl OutputPin + 'static,
        i2c: &Arc<Mutex<I2cDriver<'static>>>,
    ) -> anyhow::Result<TouchDriverFactory> {
        Ok(TouchDriverFactory {
            int_pin: Arc::new(Mutex::new(PinDriver::input(int_pin, Pull::Up)?)),
            rst_pin: PinDriver::output(rst_pin)?,
            i2c: i2c.clone(),
        })
    }

    pub fn init(&mut self)  -> anyhow::Result<()> {
        self.rst_pin.set_low()?;
        std::thread::sleep(Duration::from_millis(200));
        self.rst_pin.set_high()?;
        std::thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    pub fn read_touch(i2c: &mut I2cDriver) -> anyhow::Result<Option<TouchData>> {
        let mut buf = [0u8; 14];
        // i2c.write_read(0x63, &[0x01], &mut buf, BLOCK)?;
        // Write register address separately, then read
        i2c.write(0x63, &[0x01], BLOCK)?;
        i2c.read(0x63, &mut buf, BLOCK)?;

        let num_touches = buf[1] as usize;
        if num_touches == 0 {
            return Ok(None);
        }

        let num_touches = num_touches.min(5); // MAX_TOUCH_MAX_POINTS
        let mut points = Vec::with_capacity(num_touches);

        for i in 0..num_touches {
            let base = 2 + i * 6;
            let x = ((buf[base]     as u16 & 0x0f) << 8) | buf[base + 1] as u16;
            let y = ((buf[base + 2] as u16 & 0x0f) << 8) | buf[base + 3] as u16;
            points.push(TouchPoint { x, y });
        }

        Ok(Some(TouchData { points }))
    }

    pub fn apply_rotation(point: &TouchPoint, rotation: DisplayOrientation, 
                       width: u16, height: u16) -> TouchPoint {
        match rotation {
            DisplayOrientation::Rotate0 => TouchPoint { // rotation 0 (default)
                x: width - 1 - point.x, 
                y: point.y 
            },
            DisplayOrientation::Rotate90 => TouchPoint { 
                x: point.y,
                y: point.x, 
            },
            DisplayOrientation::Rotate180 => TouchPoint { 
                x: point.x, 
                y: height - 1 - point.y 
            },

            DisplayOrientation::Rotate270 => TouchPoint {
                x: height - 1 - point.y,
                y: width - 1 - point.x
            },
        }
    }

    pub fn add_listener(&self, listener: &Arc<dyn Listener<TouchPoint>>) {
        self.listener_manager.add_listener(listener);
    }

    pub fn remove_listener(&self, listener: &Arc<dyn Listener<TouchPoint>>) {
        self.listener_manager.remove_listener(listener);
    }

    pub fn start_touch_task(&mut self) -> anyhow::Result<()> {

        let i2c_mutex = self.i2c.clone();
        let int_pin_mutex = self.int_pin.clone();
        let rotation = self.rotation;
        let width = self.width;
        let height = self.height;

        int_pin_mutex.lock().unwrap().set_interrupt_type(InterruptType::NegEdge)?;


        // Verify chip is responding after reset
        {
            let mut i2c = i2c_mutex.lock().unwrap();
            let mut id = [0u8; 3];
            // i2c.write_read(0x63, &[0x08], &mut id, BLOCK)?;
            i2c.write(0x63, &[0x08], BLOCK)?;
            i2c.read(0x63, &mut id, BLOCK)?;
            log::info!("AXS5106L ID: {:02x} {:02x} {:02x}", id[0], id[1], id[2]);
        }

        let listener_manager = self.listener_manager.clone();

        std::thread::Builder::new()
            .stack_size(4096)
            .name("touch".into())
            .spawn(move || {
                // Created here — captures this thread's task handle
                let notification = Notification::new();
                let notifier = notification.notifier();

                // We are just going to hold the lock on the int pin for the whole time we are running.
                let mut int_pin = int_pin_mutex.lock().unwrap();

                unsafe {
                    int_pin.subscribe(move || {
                        notifier.notify(NonZeroU32::new(1).unwrap());
                    }).unwrap();
                }
                int_pin.enable_interrupt().unwrap();

                let mut min_x = 10000_u16;
                let mut max_x = 0_u16;
                let mut min_y = 10000_u16;
                let mut max_y = 0_u16;

                loop {
                    notification.wait(esp_idf_hal::delay::BLOCK);

                    FreeRtos::delay_ms(10);  // give chip time to prepare data

                    int_pin.enable_interrupt().ok();

                    let mut i2c = i2c_mutex.lock().unwrap();
                    match Self::read_touch(&mut *i2c) {
                        Ok(Some(data)) => {
                            for point in &data.points {
                                if min_x > point.x {
                                    min_x = point.x;
                                }
                                if max_x < point.x {
                                    max_x = point.x;
                                }

                                if min_y > point.y {
                                    min_y = point.y;
                                }
                                if max_y < point.y {
                                    max_y = point.y;
                                }

                                let rotated = Self::apply_rotation(point, rotation, width, height);
                                // log::info!("Rotated x={} y={} Touch x={} y={} Bounds {} {} {} {}", 
                                //     rotated.x, rotated.y,
                                //     point.x, point.y,
                                //     min_x, max_x, min_y, max_y);
                                
                                listener_manager.emit(&rotated);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => log::error!("Touch read error: {:?}", e),
                    }
                }
            })?;

        Ok(())
    }
}
