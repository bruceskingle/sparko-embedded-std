use sparko_esp_std::{binary_clock_feature::BinaryClockFeature, dyndns2::DynDns2, sparko_esp32_std::SparkoEsp32Std};

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

    let smart_leds = sparko_esp_std::smart_led::new(
        remainder.spi3,
        remainder.gpio14,       //SCLK
        remainder.gpio13,        //SDO / MISO
        64)?;

    let sparko_esp32 = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(BinaryClockFeature::new_spi(smart_leds)))?
        .build()?;
    
    log::info!("Trace 2");
    sparko_esp32.start()
}