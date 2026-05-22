
use esp_idf_hal::{gpio::AnyIOPin, spi::{Dma, SpiDeviceDriver, SpiDriver, SpiDriverConfig}, units::Hertz};
use log::info;
use sparko_esp_std::{binary_clock_feature::BinaryClockFeature, dyndns2::DynDns2, smart_led::SmartLedsSpi, sparko_esp32_std::SparkoEsp32Std};



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

// pub fn spi2_driver(mut self: &Remainder) -> anyhow::Result<SpiDriver<'static>> {
//         if self.spi2_driver.is_some() {
//             Ok(self.spi2_driver.as_mut().unwrap().clone())
//         }
//         else {
//             let driver = SpiDriver::new(
//                 self.spi2.take().unwrap(),
//                 self.gpio4.take().unwrap(),       //SCLK
//                 self.gpio5.take().unwrap(),        //SDO / MISO
//                 // Some(peripherals.pins.gpio6),   //SDI / MOSI
//                 None::<AnyIOPin>,              //SDI / MOSI
//                 &SpiDriverConfig::new()
//                     .dma(Dma::Auto(4096))
//             ).anyhow()?;
//             self.spi2_driver = Some(driver);
//             let x: &mut SpiDriver<'static> = self.spi2_driver.as_mut().unwrap();
//             let y = x.clone();
//             Ok(self.spi2_driver.as_mut().unwrap().clone())
//         }
//     }

fn run() -> anyhow::Result<()> {
    // let mut builder = SparkoCyd::Builder::new();

    // let mut sparko_cyd = builder
    //     .with_feature(DynDns2::new())
    //     .build()?;
    let (builder, mut remainder) = SparkoEsp32Std::builder()?;

    // let driver = SpiDriver::new(
    //     remainder.spi2,
    //     remainder.gpio_sclk,       //SCLK
    //     remainder.gpio_sdo_miso,        //SDO / MISO
    //     // Some(peripherals.pins.gpio_sdi_mosi),   //SDI / MOSI
    //     None::<AnyIOPin>,              //SDI / MOSI
    //     &SpiDriverConfig::new()
    //         .dma(Dma::Auto(sparko_esp_std::binary_clock_feature::required_spi_transfer_size()))
    // )?;

    // // let driver = remainder.spi2_driver()?.as_ref().unwrap();

    // // let driver: SpiDriver<'_>= spi2_driver(&mut remainder)?;
    // // let driver: SpiDriver<'_> = driver.clone();

    // let spi = SpiDeviceDriver::new(
    //     driver,
    //     None::<AnyIOPin>,   //CS / SS
    //     // Some(peripherals.pins.gpio7),   //CS / SS
    //     &esp_idf_hal::spi::config::Config::new()
    //         .baudrate(Hertz(2_400_000))
    //         .queue_size(1),
    // )?;


    let smart_leds = sparko_esp_std::smart_led::new(

        remainder.spi2,
        remainder.gpio_sclk,       //SCLK
        remainder.gpio_sdo_miso,        //SDO / MISO
        64)?;
        // remainder.rmt.channel0, remainder.gpio21, 64)?;

    let mut sparko_esp32 = builder
        .with_feature(Box::new(DynDns2::new()?))?
        // .with_feature(Box::new(BinaryClockFeature::new_rmt(remainder.rmt.ok_or(anyhow::format_err!("RMT unavailable"))?.channel0, remainder.gpio10.take().ok_or(anyhow::format_err!("GPIO 10 unavailable"))?)))?
        .with_feature(Box::new(BinaryClockFeature::new_spi(smart_leds)))?
        .build()?;

    // let mut features = Vec::<Box<dyn Feature>>::new();
    // features.push(Box::new(DynDns2::new()?));

    // log::info!("Trace 1");
    // let mut sparko_cyd = SparkoCyd::new(features)?;

    // let cloned_ap_mode = sparko_cyd.ap_mode.clone();
    // sparko_cyd.server_manager.fn_handler("/", Method::Get, move |req| {

    //         // info!("Received request for / from {}", req.connection().remote_addr());

    //         info!("Received {:?} request for {}", req.method(), req.uri());

    //         if cloned_ap_mode.lock().unwrap().clone() {
    //             let mut resp = req.into_response(
    //                 302,
    //                 Some("Found"),
    //                 &[("Location", "/config")],
    //             )?;
    //         }
    //         else {

    //             let mut resp = req.into_ok_response()?;
    //             resp.write(r#"
    //                 <!DOCTYPE html>
    //                 <html lang="en">
    //                 <head>
    //                     <meta charset="utf-8" />
    //                     <meta name="viewport" content="width=device-width, initial-scale=1" />
    //                     <title>ESP32 Home</title>
    //                     <link rel="stylesheet" href="/main.css">
    //                 </head>
    //                 <body>
    //                     <div class="page">
    //                         <h1>ESP32 Home</h1>
    //                         <p>Welcome to the ESP32 home page!</p>
    //                         <p>Current time: "#.as_bytes())?;

    //             let now = Local::now();
    //             let time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    //             resp.write(time.as_bytes())?;
    //             resp.write(r#"</p>
    //                     </div>
    //                 </body>
    //                 </html>
    //                 "#.as_bytes())?;
    //         }
    //         Ok(())
    //     })?;

    
    log::info!("Trace 2");
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


