use sparko_esp_idf::{
    binary_clock_feature::BinaryClockFeature, dyndns2::DynDns2, sparko_esp32_std::Esp32Platform,
};

fn main() -> anyhow::Result<()> {
    // let mut builder = SparkoCyd::Builder::new();

    // let mut sparko_cyd = builder
    //     .with_feature(DynDns2::new())
    //     .build()?;
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(remainder.rmt.channel0, remainder.gpio21, 64)?;
    let mut sparko_esp32 = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(BinaryClockFeature::new_rmt(smart_leds)))?
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
