
use std::{sync::{Arc, Mutex}, time::Duration};

use embedded_graphics::{prelude::{Point, Size}, primitives::Rectangle};
use log::info;
use sparko_embedded_std::{DisplayOrientation, config::ConfigSpec, feature::FeatureDescriptor, graphics::DisplayManager, listener::Listener, task::scheduler::ScheduledTask};
use sparko_esp_idf::{Feature, analog_clock_feature::AnalogClock, binary_clock_feature::BinaryClockFeature, dyndns2::DynDns2, sparko_esp32_std::SparkoEsp32Std, touch::axs5106l::TouchPoint};
use sparko_embedded_std::platform::SparkoEmbeddedStdInitializer;

struct ImuFeature {

}

impl ImuFeature {
    pub fn new() -> Self {
        Self {}
    }
}

impl Feature for ImuFeature {
    fn init(&self, init: &mut sparko_esp_idf::sparko_esp32_std::SparkoEsp32StdInitializer) -> anyhow::Result<sparko_embedded_std::feature::FeatureDescriptor>  {
        info!("ImuFeature::init");
        let config = ConfigSpec::builder()
            .build();
        
        Ok(FeatureDescriptor {
            name: "ImuFeature".to_string(),
            config,
        })
    }

    fn start(&mut self, sparko: &mut sparko_esp_idf::sparko_esp32_std::SparkoEsp32Std, initializer: &mut sparko_esp_idf::sparko_esp32_std::SparkoEsp32StdInitializer, config: &sparko_embedded_std::config::Config) -> anyhow::Result<()> {
        info!("ImuFeature::start");
        sparko.imu_manager.start(Duration::from_millis(50))?;

        info!("ImuFeature::start...add task");
        initializer.add_task(Box::new(ImuTask{
            
        }), "* * * * * *")?;

        info!("ImuFeature::start...OK");
        Ok(())
    }
}

struct ImuTask {

}

impl ScheduledTask<SparkoEsp32Std> for ImuTask
{
    // fn run(&mut self, _sparko_cyd: &dyn SparkoEmbeddedStd) -> anyhow::Result<()> {
    //     let clock_renderer = 
    // }
    
    fn name(&self) -> &str {
        "IMU Logger"
    }
    
    fn run(&mut self, sparko_embedded: &mut SparkoEsp32Std) -> anyhow::Result<()> {
        

        let attitude = sparko_embedded.imu_manager.read_attitude();
        let tilt = sparko_embedded.imu_manager.read_tilt();

        info!("IMU Logger Attitude={:?} Tilt={:?}", attitude, tilt);
        Ok(())
    }
}

struct TouchListener<DM: DisplayManager + Send + 'static> {
        display_manager: Arc<Mutex<DM>>
}

impl<DM: DisplayManager + Send + 'static> Listener<TouchPoint> for TouchListener <DM> {
    fn on_event(&self, event: &TouchPoint) {
        use embedded_graphics::prelude::RgbColor;
        use embedded_graphics::Drawable;

        let mut manager = self.display_manager.lock().unwrap();
        let color = manager.map_color(&sparko_embedded_std::graphics::Color::Green);
        let target = manager.display();

        embedded_graphics::Pixel(
            embedded_graphics::geometry::Point::new(event.x as i32, event.y as i32),
            color).draw(target);
    }
}

struct TouchDrawFeature {
    listener: Option<Arc<dyn Listener<TouchPoint>>>,
}

impl TouchDrawFeature {
    pub fn new() -> Self {
        Self {
            listener: None,
        }
    }
}

impl Feature for TouchDrawFeature {
    fn init(&self, init: &mut sparko_esp_idf::sparko_esp32_std::SparkoEsp32StdInitializer) -> anyhow::Result<FeatureDescriptor>  {
        info!("TouchListener::init");
        let config = ConfigSpec::builder()
            .build();
        
        Ok(FeatureDescriptor {
            name: "TouchListener".to_string(),
            config,
        })
    }

    fn start(&mut self, sparko: &mut sparko_esp_idf::sparko_esp32_std::SparkoEsp32Std, initializer: &mut sparko_esp_idf::sparko_esp32_std::SparkoEsp32StdInitializer, config: &sparko_embedded_std::config::Config) -> anyhow::Result<()> {

        let listener: Arc<dyn Listener<TouchPoint>>
         = Arc::new(TouchListener {
            display_manager: sparko.display_manager.clone(),
        });
        sparko.touch_driver.add_listener(&listener);

        self.listener = Some(listener);
        Ok(())
    }
}


fn main() {


    log::info!("Hello, world!");

    // This is the app level fault barrier.
    // For the moment we just unwrap and panic, but in the future we might want to attempt some sort of recovery or restart.
    match run() {
        Ok(()) => log::info!("Application finished successfully"),
        Err(e) => {
            log::error!("Application failed with error: {}", e);
            panic!("App failed");
        },
    }
}

fn run() -> anyhow::Result<()> {

    let (builder, remainder) = SparkoEsp32Std::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(
        remainder.rmt.channel0,
        remainder.gpio6,
        64)?;

    let mut sparko_esp32 = builder
        .with_feature(Box::new(TouchDrawFeature::new()))?
        .with_feature(Box::new(ImuFeature::new()))?
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(AnalogClock::builder()
            .with_layout(|rect| {
                let margin = 0;
                let size = std::cmp::min(rect.size.width, rect.size.height) - 2 * margin as u32;
                Rectangle {
                    top_left: Point { x: rect.top_left.x + margin, y: rect.top_left.y + margin },
                    size: Size { width: size, height: size },
                }
            })
            .build()?))?
        // .with_display_orientation(DisplayOrientation::Rotate270)?
        .with_display_orientation(DisplayOrientation::Rotate270)?
        .with_feature(Box::new(BinaryClockFeature::new_rmt(smart_leds)))?
        .build()?;

    // let calibration = sparko_esp32.sparko_std.imu_manager.calibrate(100)?;
    // info!("Calibration = {:?}", calibration);
    // panic!("TEST");

    // let mut sparko_esp32 = SparkoEsp32Std::builder()?
    //     .with_feature(Box::new(DynDns2::new()?))?
    //     .with_feature(Box::new(AnalogClock::builder()
    //         .with_layout(|rect| {
    //             let margin = 3;
    //             let size = std::cmp::min(rect.size.width, rect.size.height) - 2 * margin as u32;
    //             Rectangle {
    //                 top_left: Point { x: rect.top_left.x + margin, y: rect.top_left.y + margin },
    //                 size: Size { width: size, height: size },
    //             }
    //         })
    //         .build()?))?
    //     .with_display_orientation(DisplayOrientation::Rotate270)?
    //     .build()?;

    // // let mut features = Vec::<Box<dyn Feature>>::new();
    // // features.push(Box::new(DynDns2::new()?));

    // // log::info!("Trace 1");
    // // let mut sparko_cyd = SparkoCyd::new(features)?;

    // // let cloned_ap_mode = sparko_cyd.ap_mode.clone();
    // // sparko_cyd.server_manager.fn_handler("/", Method::Get, move |req| {

    // //         // info!("Received request for / from {}", req.connection().remote_addr());

    // //         info!("Received {:?} request for {}", req.method(), req.uri());

    // //         if cloned_ap_mode.lock().unwrap().clone() {
    // //             let mut resp = req.into_response(
    // //                 302,
    // //                 Some("Found"),
    // //                 &[("Location", "/config")],
    // //             )?;
    // //         }
    // //         else {

    // //             let mut resp = req.into_ok_response()?;
    // //             resp.write(r#"
    // //                 <!DOCTYPE html>
    // //                 <html lang="en">
    // //                 <head>
    // //                     <meta charset="utf-8" />
    // //                     <meta name="viewport" content="width=device-width, initial-scale=1" />
    // //                     <title>ESP32 Home</title>
    // //                     <link rel="stylesheet" href="/main.css">
    // //                 </head>
    // //                 <body>
    // //                     <div class="page">
    // //                         <h1>ESP32 Home</h1>
    // //                         <p>Welcome to the ESP32 home page!</p>
    // //                         <p>Current time: "#.as_bytes())?;

    // //             let now = Local::now();
    // //             let time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    // //             resp.write(time.as_bytes())?;
    // //             resp.write(r#"</p>
    // //                     </div>
    // //                 </body>
    // //                 </html>
    // //                 "#.as_bytes())?;
    // //         }
    // //         Ok(())
    // //     })?;

    
    // log::info!("Trace 2");
    sparko_esp32.start()
    
    // ?;
    // sparko_cyd.run()
}


    // log::info!("Trace 3");
    // let current_dns = resolve_local_dns()?;
    // info!("Current DNS resolution for home.skingle.org: {}", current_dns);

    // let addr = Arc::new(Mutex::new(current_dns));

    // // let handler_addr = addr.clone();

    // let mut cnt = 0;

    // let mut r = 64;
    // let mut g = 0;
    // let mut b = 0;
    // loop {
    //     log::info!("Top of loop");

    //     // sparko_cyd.led_manager.set_color(r,g,b)?;

    //     // let c = r;
    //     // r = b;
    //     // b = g;
    //     // g = c;

    //     if cnt < 3 {
    //         match get_public_ip_address() {
    //             Ok(public_ip) => {
    //                 cnt = cnt + 1;
    //                 if public_ip != *addr.clone().lock().unwrap() {
    //                     log::info!("Public IP changed: {} -> {}", *addr.lock().unwrap(), public_ip);
    //                     // *addr.lock()? = public_ip;
    //                 } else {
    //                     log::info!("Public IP unchanged: {}", public_ip);
    //                 }
    //             },
    //             Err(e) => {
    //                 log::error!("Failed to get public IP address: {}", e);
    //             }
    //         }
    //     }

        

    //     // let mut led = led.lock()?;
    //     // led.toggle()?;
    //     std::thread::sleep(std::time::Duration::from_secs(10));
    // }


