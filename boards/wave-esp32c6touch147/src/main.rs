use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use log::info;
use sparko_embedded_std::{
    DisplayOrientation, config::ConfigSpec, feature::FeatureDescriptor, graphics::DisplayManager,
    listener::Listener, task::scheduler::ScheduledTask,
};
use sparko_embedded_std::{config::FeatureConfig, platform::PlatformInitializer};
use sparko_esp_idf::{
    Esp32Platform, Feature,
    features::{analog_clock::AnalogClock, binary_clock::BinaryClock, dyndns2::DynDns2},
    touch::axs5106l::TouchPoint,
};

#[derive(FeatureConfig)]
struct ImuConfig {}

struct ImuFeature {}

impl ImuFeature {
    pub fn new() -> Self {
        Self {}
    }
}

impl Feature for ImuFeature {
    type Config = ImuConfig;
    fn init(
        &self,
        _init: &mut sparko_esp_idf::Esp32PlatformInitializer,
    ) -> anyhow::Result<sparko_embedded_std::feature::FeatureDescriptor> {
        info!("ImuFeature::init");

        Ok(FeatureDescriptor {
            name: "ImuFeature".to_string(),
            config: ImuConfig::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        sparko: &mut sparko_esp_idf::Esp32Platform,
        initializer: &mut sparko_esp_idf::Esp32PlatformInitializer,
        _config: ImuConfig,
    ) -> anyhow::Result<()> {
        info!("ImuFeature::start");
        sparko.imu_manager.start(Duration::from_millis(50))?;

        info!("ImuFeature::start...add task");
        initializer.add_task(Box::new(ImuTask {}), "* * * * * *")?;

        info!("ImuFeature::start...OK");
        Ok(())
    }
}

struct ImuTask {}

impl ScheduledTask<Esp32Platform> for ImuTask {
    // fn run(&mut self, _sparko_cyd: &dyn Esp32Platform) -> anyhow::Result<()> {
    //     let clock_renderer =
    // }

    fn name(&self) -> &str {
        "IMU Logger"
    }

    fn run(&mut self, sparko_embedded: &mut Esp32Platform) -> anyhow::Result<()> {
        let attitude = sparko_embedded.imu_manager.read_attitude();
        let tilt = sparko_embedded.imu_manager.read_tilt();

        info!("IMU Logger Attitude={:?} Tilt={:?}", attitude, tilt);
        Ok(())
    }
}

struct TouchListener<DM: DisplayManager + Send + 'static> {
    display_manager: Arc<Mutex<DM>>,
}

impl<DM: DisplayManager + Send + 'static> Listener<TouchPoint> for TouchListener<DM> {
    fn on_event(&self, event: &TouchPoint) {
        use embedded_graphics::Drawable;

        let mut manager = self.display_manager.lock().unwrap();
        let color = manager.map_color(&sparko_embedded_std::graphics::Color::Green);
        let target = manager.display();

        embedded_graphics::Pixel(
            embedded_graphics::geometry::Point::new(event.x as i32, event.y as i32),
            color,
        )
        .draw(target)
        .ok();
    }
}

#[derive(FeatureConfig)]
struct TouchDrawConfig {}

struct TouchDrawFeature {
    listener: Option<Arc<dyn Listener<TouchPoint>>>,
}

impl TouchDrawFeature {
    pub fn new() -> Self {
        Self { listener: None }
    }
}

impl Feature for TouchDrawFeature {
    type Config = TouchDrawConfig;
    fn init(
        &self,
        _init: &mut sparko_esp_idf::Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        info!("TouchListener::init");

        Ok(FeatureDescriptor {
            name: "TouchListener".to_string(),
            config: TouchDrawConfig::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        sparko: &mut sparko_esp_idf::Esp32Platform,
        _initializer: &mut sparko_esp_idf::Esp32PlatformInitializer,
        _config: TouchDrawConfig,
    ) -> anyhow::Result<()> {
        let listener: Arc<dyn Listener<TouchPoint>> = Arc::new(TouchListener {
            display_manager: sparko.display_manager.clone(),
        });
        sparko.touch_driver.add_listener(&listener);

        self.listener = Some(listener);
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(remainder.rmt.channel0, remainder.gpio6, 64)?;

    let sparko_esp32 = builder
        .with_feature(Box::new(TouchDrawFeature::new()))?
        .with_feature(Box::new(ImuFeature::new()))?
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(
            AnalogClock::builder()
                .with_layout(|rect| {
                    let margin = 0;
                    let size = std::cmp::min(rect.size.width, rect.size.height) - 2 * margin as u32;
                    Rectangle {
                        top_left: Point {
                            x: rect.top_left.x + margin,
                            y: rect.top_left.y + margin,
                        },
                        size: Size {
                            width: size,
                            height: size,
                        },
                    }
                })
                .build()?,
        ))?
        .with_display_orientation(DisplayOrientation::Rotate270)?
        .with_feature(Box::new(BinaryClock::new_rmt(smart_leds)))?
        .build()?;

    sparko_esp32.start()
}
