use sparko_esp_idf::{
    binary_clock_feature::BinaryClockFeature, dyndns2::DynDns2, sparko_esp32_std::Esp32Platform,
};

fn main() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(
        remainder.spi3,
        remainder.gpio14, //SCLK
        remainder.gpio13, //SDO / MISO
        64,
    )?;

    let sparko_esp32 = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(BinaryClockFeature::new_spi(smart_leds)))?
        .build()?;

    sparko_esp32.start()
}
