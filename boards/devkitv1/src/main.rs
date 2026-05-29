use sparko_esp_idf::{
    Esp32Platform,
    features::{binary_clock::BinaryClock, dyndns2::DynDns2},
};

fn main() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(
        remainder.spi3,
        remainder.gpio14, //SCLK
        remainder.gpio13, //SDO / MISO
        64,
    )?;

    let platform = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(BinaryClock::new_spi(smart_leds)))?
        .build()?;

    platform.start()
}
