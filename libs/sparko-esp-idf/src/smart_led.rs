use crate::AnyhowResultExt;
use core::time::Duration;
use esp_idf_hal::gpio::AnyIOPin;
use esp_idf_hal::rmt::{PinState, Pulse, PulseTicks, Signal, Symbol};
use esp_idf_hal::spi::{Dma, SpiAnyPins, SpiDeviceDriver, SpiDriver, SpiDriverConfig};
use esp_idf_hal::units::Hertz;
use esp_idf_hal::{
    gpio::OutputPin,
    rmt::{RmtChannel, TxRmtDriver, config::TransmitConfig},
};
use esp_idf_sys::rmt_item32_t;
use rgb::RGB;
use std::borrow::Borrow;

/// T0H duration time (0 code, high voltage time)
const WS2812_T0H_NS: Duration = Duration::from_nanos(300);
/// T0L duration time (0 code, low voltage time)
const WS2812_T0L_NS: Duration = Duration::from_nanos(900);
/// T1H duration time (1 code, high voltage time)
const WS2812_T1H_NS: Duration = Duration::from_nanos(900);
/// T1L duration time (1 code, low voltage time)
const WS2812_T1L_NS: Duration = Duration::from_nanos(300);
/// Reset code duration time (low voltage time)
const WS2812_RESET_NS: Duration = Duration::from_micros(80);

pub type ColourSignal = [rmt_item32_t; 24];

pub trait SmartLeds {
    fn set_pixel_rgb(&mut self, index: usize, color: RGB<u8>) -> anyhow::Result<()>;
    fn send(&mut self) -> anyhow::Result<()>;
    fn get_num_leds(&self) -> usize;
}

#[cfg(feature = "smart-led-spi")]
/// Constructor for the default SmartLeds implementation for the current board.
/// There are two implementations one of which creates an SPI based implementation and the other which creates an RMT
/// based version. If the currently selected board supports the necessary RMT capabilities for reliable operation
/// then the RMT version is in scope, otherwise the SPI version will be. An application is free to call either
/// implementation's constructor directly (via ```SmartLedsRmt::new(...)``` or ```SmartLedsSpi::new(...)```)
/// but this method is provided for cases where the caller just wants to find the best option. Note that the parameters
/// for the two implementations are different, but IDE auto complete should make that easy to deal with.
///
/// SPI implementations
///
/// # Arguments
/// * `spi` - The SPI peripheral instance to use for communication with the LED strip
/// * `sclk` - The pin to use for the SPI clock signal
/// * `sdo` - The pin to use for the SPI data output signal
/// * `num_leds` - The number of LEDs in the strip
pub fn new<'d, SPI: SpiAnyPins + 'd>(
    spi: SPI,
    sclk: impl OutputPin + 'd,
    sdo: impl OutputPin + 'd,
    num_leds: usize,
) -> anyhow::Result<SmartLedsSpi<'d, SpiDriver<'d>>> {
    SmartLedsSpi::new(spi, sclk, sdo, num_leds)
}

#[cfg(not(feature = "smart-led-spi"))]
/// Constructor for the default SmartLeds implementation for the current board.
/// There are two implementations one of which creates an SPI based implementation and the other which creates an RMT
/// based version. If the currently selected board supports the necessary RMT capabilities for reliable operation
/// then the RMT version is in scope, otherwise the SPI version will be. An application is free to call either
/// implementation's constructor directly (via ```SmartLedsRmt::new(...)``` or ```SmartLedsSpi::new(...)```)
/// but this method is provided for cases where the caller just wants to find the best option. Note that the parameters
/// for the two implementations are different, but IDE auto complete should make that easy to deal with.
///
/// SPI implementations
///
/// # Arguments
/// * `channel` - The RmtChannel to use for timing
/// * `pin` - The pin to use for the data output signal
pub fn new<'d, C: RmtChannel + 'd>(
    channel: C,
    pin: impl OutputPin + 'd,
    num_leds: usize,
) -> anyhow::Result<SmartLedsRmt<'d>> {
    SmartLedsRmt::new(channel, pin, num_leds)
}

pub struct SmartLedsRmt<'a> {
    rmt: TxRmtDriver<'a>,
    buf: Vec<rmt_item32_t>,
    num_leds: usize,
    zero_signal: rmt_item32_t,
    one_signal: rmt_item32_t,
    reset_signal: rmt_item32_t,
}

impl<'a> SmartLedsRmt<'a> {
    pub fn new<C: RmtChannel + 'a>(
        channel: C,
        pin: impl OutputPin + 'a,
        num_leds: usize,
    ) -> anyhow::Result<Self> {
        let config = TransmitConfig::new()
            .clock_divider(8)
            .carrier(None)
            .idle(Some(PinState::Low));
        let rmt: TxRmtDriver<'a> = TxRmtDriver::new(channel, pin, &config).unwrap();

        let ticks_hz = rmt.counter_clock().anyhow()?;

        let zero_symbol = Symbol::new(
            Pulse::new(
                PinState::High,
                PulseTicks::new_with_duration(ticks_hz, &WS2812_T0H_NS).anyhow()?,
            ),
            Pulse::new(
                PinState::Low,
                PulseTicks::new_with_duration(ticks_hz, &WS2812_T0L_NS).anyhow()?,
            ),
        );

        let zero_signal = zero_symbol.as_slice()[0]; // Get the rmt_item32_t representation of the zero symbol

        let one_symbol = Symbol::new(
            Pulse::new(
                PinState::High,
                PulseTicks::new_with_duration(ticks_hz, &WS2812_T1H_NS).anyhow()?,
            ),
            Pulse::new(
                PinState::Low,
                PulseTicks::new_with_duration(ticks_hz, &WS2812_T1L_NS).anyhow()?,
            ),
        );
        let one_signal = one_symbol.as_slice()[0]; // Get the rmt_item32_t representation of the one symbol

        let reset_symbol = Symbol::new(
            Pulse::new(
                PinState::Low,
                PulseTicks::new_with_duration(ticks_hz, &WS2812_RESET_NS).anyhow()?,
            ),
            Pulse::new(PinState::Low, PulseTicks::zero()),
        );
        let reset_signal = reset_symbol.as_slice()[0]; // Get the rmt_item32_t representation of the reset symbol

        let mut buf = Vec::with_capacity(24 * num_leds + 1); // Buffer for up to 100 LEDs (24 bits per LED + reset pulse)
        for _ in 0..(24 * num_leds) {
            buf.push(zero_signal);
        }
        buf.push(reset_signal);
        Ok(Self {
            rmt,
            buf,
            num_leds,
            zero_signal,
            one_signal,
            reset_signal,
        })
    }

    pub fn to_signal(&self, pixel: RGB<u8>) -> ColourSignal {
        let mut signal = [self.zero_signal; 24];
        for (i, &byte) in [pixel.g, pixel.r, pixel.b].iter().enumerate() {
            for bit in 0..8 {
                let bit_is_set = (byte >> (7 - bit)) & 1 == 1;
                signal[i * 8 + bit] = if bit_is_set {
                    self.one_signal
                } else {
                    self.zero_signal
                };
            }
        }
        signal
    }

    pub fn set_pixel_signal(&mut self, index: usize, signal: ColourSignal) -> anyhow::Result<()> {
        if index >= self.num_leds {
            return Err(anyhow::anyhow!("Pixel index out of bounds"));
        }

        let start = index * 24;
        for i in 0..24 {
            self.buf[start + i] = signal[i];
        }
        Ok(())
    }
}

impl SmartLeds for SmartLedsRmt<'_> {
    fn set_pixel_rgb(&mut self, index: usize, color: RGB<u8>) -> anyhow::Result<()> {
        self.set_pixel_signal(index, self.to_signal(color))
    }

    fn send(&mut self) -> anyhow::Result<()> {
        // self.rmt.start_blocking(self.buf.as_slice()).unwrap();
        let buf = self.buf.clone();
        self.rmt.start_blocking(buf.as_slice()).anyhow()?;
        Ok(())
    }

    fn get_num_leds(&self) -> usize {
        self.num_leds
    }
}

pub struct SmartLedsSpi<'d, T>
where
    T: Borrow<SpiDriver<'d>> + 'd,
{
    spi: SpiDeviceDriver<'d, T>,
    buf: Vec<u8>,
    num_leds: usize,
    p: std::marker::PhantomData<&'d ()>,
}

// pub fn new_func<'d, SPI: SpiAnyPins + 'd>(
//     spi: SPI,
//     sclk: impl OutputPin + 'd,
//     sdo: impl OutputPin + 'd,
//     // spi: SpiDeviceDriver<'d, T>,
//     num_leds: usize,
// ) -> anyhow::Result<SmartLedsSpi<'d, _>> {

//     let driver = SpiDriver::new(
//         spi,sclk,
//         sdo,
//         None::<AnyIOPin>,              //SDI / MOSI
//         &SpiDriverConfig::new()
//             .dma(Dma::Auto(SmartLedsSpi::required_spi_transfer_size(num_leds)))
//     )?;

//     // let driver = remainder.spi2_driver()?.as_ref().unwrap();

//     // let driver: SpiDriver<'_>= spi2_driver(&mut remainder)?;
//     // let driver: SpiDriver<'_> = driver.clone();

//     let spi = SpiDeviceDriver::new(
//         driver,
//         None::<AnyIOPin>,   //CS / SS
//         // Some(peripherals.pins.gpio7),   //CS / SS
//         &esp_idf_hal::spi::config::Config::new()
//             .baudrate(Hertz(2_400_000))
//             .queue_size(1),
//     )?;

//     SmartLedsSpi::from_spi(spi, num_leds)
// }

impl<'d> SmartLedsSpi<'d, SpiDriver<'d>> {
    pub fn new<SPI: SpiAnyPins + 'd>(
        spi: SPI,
        sclk: impl OutputPin + 'd,
        sdo: impl OutputPin + 'd,
        num_leds: usize,
    ) -> anyhow::Result<Self> {
        let driver = SpiDriver::new(
            spi,
            sclk,
            sdo,
            None::<AnyIOPin>,
            &SpiDriverConfig::new().dma(Dma::Auto(Self::required_spi_transfer_size(num_leds))),
        )?;

        let spi = SpiDeviceDriver::new(
            driver,
            None::<AnyIOPin>,
            &esp_idf_hal::spi::config::Config::new()
                .baudrate(Hertz(2_400_000))
                .queue_size(1),
        )?;

        Self::from_spi(spi, num_leds)
    }
}

impl<'d, T> SmartLedsSpi<'d, T>
where
    T: Borrow<SpiDriver<'d>> + 'd,
{
    /// This is the required size of a single dma transfer, effectively the required dma buffer size
    pub fn required_spi_transfer_size(num_leds: usize) -> usize {
        ((num_leds * 9 + 50) / 4 + 1) * 4
    }

    // pub fn new<SPI: SpiAnyPins + 'd>(
    //     spi: SPI,
    //     sclk: impl OutputPin + 'd,
    //     sdo: impl OutputPin + 'd,
    //     // spi: SpiDeviceDriver<'d, T>,
    //     num_leds: usize,
    // ) -> anyhow::Result<Self> {

    //     let driver = SpiDriver::new(
    //         spi,sclk,
    //         sdo,
    //         None::<AnyIOPin>,              //SDI / MOSI
    //         &SpiDriverConfig::new()
    //             .dma(Dma::Auto(Self::required_spi_transfer_size(num_leds)))
    //     )?;

    //     // let driver = remainder.spi2_driver()?.as_ref().unwrap();

    //     // let driver: SpiDriver<'_>= spi2_driver(&mut remainder)?;
    //     // let driver: SpiDriver<'_> = driver.clone();

    //     let spi = SpiDeviceDriver::new(
    //         driver,
    //         None::<AnyIOPin>,   //CS / SS
    //         // Some(peripherals.pins.gpio7),   //CS / SS
    //         &esp_idf_hal::spi::config::Config::new()
    //             .baudrate(Hertz(2_400_000))
    //             .queue_size(1),
    //     )?;

    //     Self::from_spi(spi, num_leds)
    // }

    pub fn from_spi(spi: SpiDeviceDriver<'d, T>, num_leds: usize) -> anyhow::Result<Self> {
        let capacity = num_leds * 9 + 50;
        // 9 bytes per LED (24 bits * 3 SPI bits / 8)

        let mut buf = Vec::with_capacity(capacity); //vec![0u8; capacity]; // + reset padding
        for _ in 0..(capacity) {
            buf.push(0u8);
        }

        let mut smartleds = Self {
            spi,
            buf,
            num_leds,
            p: std::marker::PhantomData,
        };

        for pixel in 0..num_leds {
            smartleds.set_pixel_rgb(pixel, RGB { r: 0, g: 0, b: 0 })?;
        }

        Ok(smartleds)
    }

    #[inline]
    fn encode_byte(byte: u8, out: &mut [u8]) {
        let mut bit_pos = 0;

        for i in 0..8 {
            let bit = (byte >> (7 - i)) & 1;

            let pattern = if bit == 1 { 0b110 } else { 0b100 };

            for j in 0..3 {
                if (pattern >> (2 - j)) & 1 == 1 {
                    let byte_index = bit_pos / 8;
                    let bit_index = 7 - (bit_pos % 8);
                    out[byte_index] |= 1 << bit_index;
                }
                bit_pos += 1;
            }
        }
    }
}

impl<'d, T> SmartLeds for SmartLedsSpi<'d, T>
where
    T: Borrow<SpiDriver<'d>> + 'd,
{
    fn set_pixel_rgb(&mut self, index: usize, color: RGB<u8>) -> anyhow::Result<()> {
        let start = index * 9;
        let slice = &mut self.buf[start..start + 9];

        // Clear
        for b in slice.iter_mut() {
            *b = 0;
        }

        // WS2812 expects GRB order
        Self::encode_byte(color.g, &mut slice[0..3]);
        Self::encode_byte(color.r, &mut slice[3..6]);
        Self::encode_byte(color.b, &mut slice[6..9]);
        Ok(())
    }

    fn send(&mut self) -> anyhow::Result<()> {
        let buf = self.buf.clone();
        self.spi.write(buf.as_slice())?;
        Ok(())
    }

    fn get_num_leds(&self) -> usize {
        self.num_leds
    }
}
