use sparko_esp_idf::Esp32Platform;
use sparko_esp_idf::features::analog_clock::AnalogClock;
use sparko_esp_idf::features::binary_clock::BinaryClock;
use sparko_esp_idf::features::dyndns2::DynDns2;

fn main() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = sparko_esp_idf::smart_led::new(
        remainder.spi3,
        remainder.gpio27, //SCLK
        remainder.gpio22, //SDO / MISO
        64,
    )?;

    let sparko_esp32 = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(AnalogClock::builder().build()?))?
        .with_feature(Box::new(BinaryClock::new_spi(smart_leds)))?
        .with_display_orientation(sparko_embedded_std::DisplayOrientation::Rotate0)?
        .build()?;

    sparko_esp32.start()
}
