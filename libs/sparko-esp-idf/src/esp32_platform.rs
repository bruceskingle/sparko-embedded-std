use crate::DynFeature;
use crate::Feature;
use crate::commands::EspCommands;
use crate::config_store::EspConfigStoreFactory;
use crate::http::EspHttpServerManager;
use crate::{core::Core, wifi::WiFiManager};
use chrono::Local;
use esp_idf_svc::sntp::*;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::peripherals::Peripherals, http::Method,
    nvs::EspDefaultNvsPartition,
};
use esp_idf_sys::*;
use log::{error, info};
use sparko_embedded_std::config::Config;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config_manager::{ConfigManager, ConfigManagerBuilder};
use sparko_embedded_std::http_server::HttpServerManager;
use sparko_embedded_std::platform::{Platform, PlatformInitializer};
use sparko_embedded_std::{InitStatus, Status};
use sparko_embedded_std::{
    problem::ProblemManager,
    task::scheduler::{ScheduledTask, TaskScheduler, TaskSchedulerBuilder},
};
use std::ffi::CStr;
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::{
    net::UdpSocket,
    sync::{Arc, Mutex},
    thread,
};

#[cfg(feature = "mipi-dsi-display")]
use crate::display::mipi_dsi_display_manager;
#[cfg(feature = "mono-led")]
use crate::led::mono_led::MonoLedManager;
#[cfg(feature = "rgb-led")]
use crate::led::rgb_led::RgbLedManager;
#[cfg(feature = "simple-led")]
use crate::led::simple_led::SimpleLedManager;
#[cfg(feature = "board-supermini-esp32c3")]
use esp_idf_hal::gpio::AnyIOPin;
#[cfg(feature = "mipi-dsi-display")]
use esp_idf_hal::gpio::{Output, PinDriver};
#[cfg(feature = "board-supermini-esp32c3")]
use esp_idf_hal::spi::{Dma, SpiDeviceDriver, SpiDriver, SpiDriverConfig};
#[cfg(feature = "mipi-dsi-display")]
use esp_idf_hal::spi::{SpiDeviceDriver, SpiDriver};
#[cfg(feature = "display")]
use sparko_embedded_std::{DisplayOrientation, graphics::DisplayManager};

#[cfg(feature = "led")]
use crate::led::LedManager;

#[cfg(feature = "touch-driver")]
use crate::touch::axs5106l::{TouchDriver, TouchDriverFactory};

fn list_nvs_keys() {
    info!("Listing NVS keys:");
    unsafe {
        let mut it: nvs_iterator_t = std::ptr::null_mut();
        let part = CStr::from_bytes_with_nul_unchecked(b"nvs\0");

        let res = nvs_entry_find(
            part.as_ptr(), // partition name
            // std::ptr::null(), // partition
            std::ptr::null(), // namespace
            nvs_type_t_NVS_TYPE_ANY,
            &mut it,
        );

        if res == ESP_OK {
            info!("NVS keys found:");
            while !it.is_null() {
                let mut info: nvs_entry_info_t = std::mem::zeroed();

                nvs_entry_info(it, &mut info);

                let namespace = CStr::from_ptr(info.namespace_name.as_ptr())
                    .to_str()
                    .unwrap();

                let key = CStr::from_ptr(info.key.as_ptr()).to_str().unwrap();

                info!("NS: {}, Key: {}", namespace, key);

                nvs_entry_next(&mut it);
            }

            nvs_release_iterator(it);
        } else {
            info!("Failed to list NVS keys: {}", res);
        }
        info!("Finished listing NVS keys");
    }
}

pub struct Esp32PlatformInitializer {
    task_manager_builder: TaskSchedulerBuilder<Esp32Platform>,
}

impl Esp32PlatformInitializer {
    fn new() -> Self {
        Self {
            task_manager_builder: TaskScheduler::builder(),
        }
    }

    // pub fn build(mut self) -> anyhow::Result<Esp32Platform> {
    //     self.features.shrink_to_fit();
    //     Esp32Platform::new(self.features, self.task_manager_builder.build())
    // }
}

impl PlatformInitializer for Esp32PlatformInitializer {
    type Platform = Esp32Platform;

    fn add_task(
        &mut self,
        task_initializer: Box<dyn ScheduledTask<Esp32Platform>>,
        schedule_spec: &str,
    ) -> anyhow::Result<()> {
        self.task_manager_builder
            .add_task(task_initializer, schedule_spec)?;
        Ok(())
    }
}

/* New board template

pub struct Remainder {
    pub gpio0: esp_idf_hal::gpio::Gpio0<'static>,
    pub gpio1: esp_idf_hal::gpio::Gpio1<'static>,
    pub gpio2: esp_idf_hal::gpio::Gpio2<'static>,
    pub gpio3: esp_idf_hal::gpio::Gpio3<'static>,
    pub gpio4: esp_idf_hal::gpio::Gpio4<'static>,
    pub gpio5: esp_idf_hal::gpio::Gpio5<'static>,
    pub gpio6: esp_idf_hal::gpio::Gpio6<'static>,
    pub gpio7: esp_idf_hal::gpio::Gpio7<'static>,
    pub gpio8: esp_idf_hal::gpio::Gpio8<'static>,
    pub gpio9: esp_idf_hal::gpio::Gpio9<'static>,


    pub gpio10: esp_idf_hal::gpio::Gpio10<'static>,
    pub gpio11: esp_idf_hal::gpio::Gpio11<'static>,
    pub gpio12: esp_idf_hal::gpio::Gpio12<'static>,
    pub gpio13: esp_idf_hal::gpio::Gpio13<'static>,
    pub gpio14: esp_idf_hal::gpio::Gpio14<'static>,
    pub gpio15: esp_idf_hal::gpio::Gpio15<'static>,
    pub gpio16: esp_idf_hal::gpio::Gpio16<'static>,
    pub gpio17: esp_idf_hal::gpio::Gpio17<'static>,
    pub gpio18: esp_idf_hal::gpio::Gpio18<'static>,
    pub gpio19: esp_idf_hal::gpio::Gpio19<'static>,

    pub gpio20: esp_idf_hal::gpio::Gpio20<'static>,
    pub gpio21: esp_idf_hal::gpio::Gpio21<'static>,
    pub gpio22: esp_idf_hal::gpio::Gpio22<'static>,
    pub gpio23: esp_idf_hal::gpio::Gpio23<'static>,
    pub gpio24: esp_idf_hal::gpio::Gpio24<'static>,
    pub gpio25: esp_idf_hal::gpio::Gpio25<'static>,
    pub gpio26: esp_idf_hal::gpio::Gpio26<'static>,
    pub gpio27: esp_idf_hal::gpio::Gpio27<'static>,
    pub gpio28: esp_idf_hal::gpio::Gpio28<'static>,
    pub gpio29: esp_idf_hal::gpio::Gpio29<'static>,

    pub gpio30: esp_idf_hal::gpio::Gpio30<'static>,
    pub gpio31: esp_idf_hal::gpio::Gpio31<'static>,
    pub gpio32: esp_idf_hal::gpio::Gpio32<'static>,
    pub gpio33: esp_idf_hal::gpio::Gpio33<'static>,
    pub gpio34: esp_idf_hal::gpio::Gpio34<'static>,
    pub gpio35: esp_idf_hal::gpio::Gpio35<'static>,
    pub gpio36: esp_idf_hal::gpio::Gpio36<'static>,
    pub gpio37: esp_idf_hal::gpio::Gpio37<'static>,
    pub gpio38: esp_idf_hal::gpio::Gpio38<'static>,
    pub gpio39: esp_idf_hal::gpio::Gpio39<'static>,

    pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    pub i2c1: esp_idf_hal::i2c::I2C1<'static>,
    pub spi2: esp_idf_hal::spi::SPI2<'static>,
    pub spi3: esp_idf_hal::spi::SPI3<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
}

*/

#[cfg(feature = "board-cyd")]
pub struct Remainder {
    pub gpio22: esp_idf_hal::gpio::Gpio22<'static>,
    pub gpio27: esp_idf_hal::gpio::Gpio27<'static>,
    pub gpio35: esp_idf_hal::gpio::Gpio35<'static>,
    // pub lpwr: esp_idf_hal::peripherals:: LPWR<'static>,
    // pub rmt: esp_hal::peripherals::RMT<'static>,
    // pub timg0: esp_hal::peripherals::TIMG0<'static>,
    pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    pub i2c1: esp_idf_hal::i2c::I2C1<'static>,
    pub spi3: esp_idf_hal::spi::SPI3<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
}

#[cfg(feature = "board-wave-esp32c6147")]
pub struct Remainder {
    pub gpio0: esp_idf_hal::gpio::Gpio0<'static>,
    pub gpio1: esp_idf_hal::gpio::Gpio1<'static>,
    pub gpio2: esp_idf_hal::gpio::Gpio2<'static>,
    pub gpio3: esp_idf_hal::gpio::Gpio3<'static>,
    pub gpio4: esp_idf_hal::gpio::Gpio4<'static>,
    pub gpio5: esp_idf_hal::gpio::Gpio5<'static>,

    pub gpio9: esp_idf_hal::gpio::Gpio9<'static>,

    pub gpio12: esp_idf_hal::gpio::Gpio12<'static>,
    pub gpio13: esp_idf_hal::gpio::Gpio13<'static>,

    pub gpio18: esp_idf_hal::gpio::Gpio18<'static>,
    pub gpio19: esp_idf_hal::gpio::Gpio19<'static>,

    pub gpio20: esp_idf_hal::gpio::Gpio20<'static>,
    pub gpio23: esp_idf_hal::gpio::Gpio23<'static>,

    pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
}

#[cfg(feature = "board-wave-esp32c6touch147")]
pub struct Remainder {
    // pub gpio0: esp_idf_hal::gpio::Gpio0<'static>,
    // pub gpio1: esp_idf_hal::gpio::Gpio1<'static>,
    // pub gpio2: esp_idf_hal::gpio::Gpio2<'static>,
    // pub gpio3: esp_idf_hal::gpio::Gpio3<'static>,
    pub gpio4: esp_idf_hal::gpio::Gpio4<'static>,
    pub gpio5: esp_idf_hal::gpio::Gpio5<'static>,
    pub gpio6: esp_idf_hal::gpio::Gpio6<'static>,
    pub gpio7: esp_idf_hal::gpio::Gpio7<'static>,
    pub gpio8: esp_idf_hal::gpio::Gpio8<'static>,
    pub gpio9: esp_idf_hal::gpio::Gpio9<'static>,

    // pub gpio10: esp_idf_hal::gpio::Gpio10<'static>,
    // pub gpio11: esp_idf_hal::gpio::Gpio11<'static>,
    // pub gpio12: esp_idf_hal::gpio::Gpio12<'static>,
    // pub gpio13: esp_idf_hal::gpio::Gpio13<'static>,
    // pub gpio14: esp_idf_hal::gpio::Gpio14<'static>,
    // pub gpio15: esp_idf_hal::gpio::Gpio15<'static>,
    // pub gpio16: esp_idf_hal::gpio::Gpio16<'static>,
    // pub gpio17: esp_idf_hal::gpio::Gpio17<'static>,
    // pub gpio18: esp_idf_hal::gpio::Gpio18<'static>,
    // pub gpio19: esp_idf_hal::gpio::Gpio19<'static>,

    // pub gpio20: esp_idf_hal::gpio::Gpio20<'static>,
    // pub gpio21: esp_idf_hal::gpio::Gpio21<'static>,
    // pub gpio22: esp_idf_hal::gpio::Gpio22<'static>,
    // pub gpio23: esp_idf_hal::gpio::Gpio23<'static>,
    // pub gpio24: esp_idf_hal::gpio::Gpio24<'static>,
    // pub gpio25: esp_idf_hal::gpio::Gpio25<'static>,
    // pub gpio26: esp_idf_hal::gpio::Gpio26<'static>,
    // pub gpio27: esp_idf_hal::gpio::Gpio27<'static>,
    // pub gpio28: esp_idf_hal::gpio::Gpio28<'static>,
    // pub gpio29: esp_idf_hal::gpio::Gpio29<'static>,

    // pub gpio30: esp_idf_hal::gpio::Gpio30<'static>,
    // pub gpio31: esp_idf_hal::gpio::Gpio31<'static>,
    // pub gpio32: esp_idf_hal::gpio::Gpio32<'static>,
    // pub gpio33: esp_idf_hal::gpio::Gpio33<'static>,
    // pub gpio34: esp_idf_hal::gpio::Gpio34<'static>,
    // pub gpio35: esp_idf_hal::gpio::Gpio35<'static>,
    // pub gpio36: esp_idf_hal::gpio::Gpio36<'static>,
    // pub gpio37: esp_idf_hal::gpio::Gpio37<'static>,
    // pub gpio38: esp_idf_hal::gpio::Gpio38<'static>,
    // pub gpio39: esp_idf_hal::gpio::Gpio39<'static>,

    // pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    // pub i2c1: esp_idf_hal::i2c::I2C1<'static>,
    // pub spi2: esp_idf_hal::spi::SPI2<'static>,
    // pub spi3: esp_idf_hal::spi::SPI3<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
}

#[cfg(feature = "board-xiao-esp32c6")]
pub struct Remainder {
    pub gpio2: esp_idf_hal::gpio::Gpio2<'static>,
    pub gpio21: esp_idf_hal::gpio::Gpio21<'static>,
    // pub lpwr: esp_idf_hal::peripherals:: LPWR<'static>,
    // pub rmt: esp_hal::peripherals::RMT<'static>,
    // pub timg0: esp_hal::peripherals::TIMG0<'static>,
    pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
}

#[cfg(feature = "board-devkitv1")]
pub struct Remainder {
    pub gpio4: esp_idf_hal::gpio::Gpio4<'static>,
    pub gpio13: esp_idf_hal::gpio::Gpio13<'static>,
    pub gpio14: esp_idf_hal::gpio::Gpio14<'static>,
    pub gpio16: esp_idf_hal::gpio::Gpio16<'static>,
    pub gpio17: esp_idf_hal::gpio::Gpio17<'static>,
    pub gpio18: esp_idf_hal::gpio::Gpio18<'static>,
    pub gpio19: esp_idf_hal::gpio::Gpio19<'static>,
    pub gpio21: esp_idf_hal::gpio::Gpio21<'static>,
    pub gpio22: esp_idf_hal::gpio::Gpio22<'static>,
    pub gpio23: esp_idf_hal::gpio::Gpio23<'static>,
    pub gpio25: esp_idf_hal::gpio::Gpio25<'static>,
    pub gpio26: esp_idf_hal::gpio::Gpio26<'static>,
    pub gpio27: esp_idf_hal::gpio::Gpio27<'static>,
    pub gpio32: esp_idf_hal::gpio::Gpio32<'static>,
    pub gpio33: esp_idf_hal::gpio::Gpio33<'static>,
    pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    pub i2c1: esp_idf_hal::i2c::I2C1<'static>,
    pub spi3: esp_idf_hal::spi::SPI3<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
}

#[cfg(feature = "board-supermini-esp32c3")]
pub struct Remainder {
    pub gpio0: esp_idf_hal::gpio::Gpio0<'static>,
    pub gpio1: esp_idf_hal::gpio::Gpio1<'static>,
    pub gpio3: esp_idf_hal::gpio::Gpio3<'static>,
    pub gpio_sclk: esp_idf_hal::gpio::Gpio4<'static>,
    pub gpio_sdo_miso: esp_idf_hal::gpio::Gpio5<'static>,
    pub gpio_sdi_mosi: esp_idf_hal::gpio::Gpio6<'static>,
    pub gpio_cs_ss: esp_idf_hal::gpio::Gpio7<'static>,
    pub gpio10: esp_idf_hal::gpio::Gpio10<'static>,
    pub gpio20: esp_idf_hal::gpio::Gpio20<'static>,
    pub gpio21: esp_idf_hal::gpio::Gpio21<'static>,
    // pub lpwr: esp_idf_hal::peripherals:: LPWR<'static>,
    // pub rmt: esp_hal::peripherals::RMT<'static>,
    // pub timg0: esp_hal::peripherals::TIMG0<'static>,
    pub i2c0: esp_idf_hal::i2c::I2C0<'static>,
    pub rmt: esp_idf_hal::rmt::RMT,
    pub spi2: esp_idf_hal::spi::SPI2<'static>,
}

pub struct Esp32PlatformBuilder {
    nvs_partition: esp_idf_svc::nvs::EspNvsPartition<esp_idf_svc::nvs::NvsDefault>,
    // failure_reason: Arc<Mutex<Option<String>>>,
    problem_manager: Arc<ProblemManager>,
    ap_mode: Arc<Mutex<bool>>,
    config_manager_builder: ConfigManagerBuilder,
    features: Vec<FeatureHolder>,
    initializer: Esp32PlatformInitializer,

    core_feature: Core,
    core_feature_name: String,
    core_config_valid: bool,
    core_feature_config: Config,
    wifi_sender: Sender<Ipv4Addr>,
    modem: esp_idf_hal::modem::Modem<'static>,
    #[cfg(feature = "display")]
    orientation: DisplayOrientation,
    #[cfg(feature = "rgb-led")]
    pub led_manager: RgbLedManager<'static>,
    #[cfg(feature = "mono-led")]
    pub led_manager: MonoLedManager,
    #[cfg(feature = "simple-led")]
    pub led_manager: SimpleLedManager<'static>,

    #[cfg(feature = "mipi-dsi-display")]
    spi: SpiDeviceDriver<'static, SpiDriver<'static>>,
    #[cfg(feature = "mipi-dsi-display")]
    dc: PinDriver<'static, Output>,
    #[cfg(feature = "mipi-dsi-display")]
    back_light: PinDriver<'static, esp_idf_hal::gpio::Output>,
    #[cfg(feature = "mipi-dsi-display-reset")]
    reset: PinDriver<'static, Output>,
    #[cfg(feature = "i2c")]
    i2c: Arc<Mutex<esp_idf_hal::i2c::I2cDriver<'static>>>,
    #[cfg(feature = "touch-driver")]
    touch_driver_factory: TouchDriverFactory,
}

impl Esp32PlatformBuilder {
    fn new() -> anyhow::Result<(Self, Remainder)> {
        // It is necessary to call this function once. Otherwise, some patches to the runtime
        // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
        esp_idf_svc::sys::link_patches();

        // Bind the log crate to the ESP Logging facilities
        esp_idf_svc::log::EspLogger::initialize_default();

        let nvs_partition: esp_idf_svc::nvs::EspNvsPartition<esp_idf_svc::nvs::NvsDefault> =
            EspDefaultNvsPartition::take()?;
        // let failure_reason: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let problem_manager = ProblemManager::new();
        let ap_mode = Arc::new(Mutex::new(false));

        list_nvs_keys();

        let config_store_factory =
            EspConfigStoreFactory::new(nvs_partition.clone(), problem_manager.clone())?;
        let mut config_manager_builder = ConfigManager::builder(
            Box::new(config_store_factory),
            problem_manager.clone(),
            ap_mode.clone(),
            Box::new(EspCommands {}),
        )?;

        let mut initializer = Esp32PlatformInitializer::new();
        let (wifi_sender, wifi_receiver): (
            Sender<std::net::Ipv4Addr>,
            Receiver<std::net::Ipv4Addr>,
        ) = mpsc::channel();
        let core_feature = Core::new(wifi_receiver)?;
        let descriptor = core_feature.init(&mut initializer)?;
        let core_feature_name = descriptor.name.clone();
        let (core_feature_config, core_config_valid) =
            config_manager_builder.add_feature(descriptor, true)?;

        let peripherals = Peripherals::take()?;
        let modem = peripherals.modem;

        #[cfg(feature = "rgb-led")]
        let led_manager = RgbLedManager::new(
            true,
            32,
            peripherals.ledc.timer0,
            peripherals.ledc.channel0,
            peripherals.pins.gpio4,
            peripherals.ledc.channel1,
            peripherals.pins.gpio16,
            peripherals.ledc.channel2,
            peripherals.pins.gpio17,
        )?;

        #[cfg(feature = "board-xiao-esp32c6")]
        let led_manager = MonoLedManager::new(true, peripherals.pins.gpio15)?;
        #[cfg(feature = "board-devkitv1")]
        let led_manager = MonoLedManager::new(false, peripherals.pins.gpio2)?;
        #[cfg(feature = "board-supermini-esp32c3")]
        let led_manager = MonoLedManager::new(false, peripherals.pins.gpio8)?;

        #[cfg(feature = "led")]
        led_manager.set_status(&Status::Initializing(InitStatus::Starting))?;

        #[cfg(feature = "board-cyd")]
        let (remainder, spi, dc, back_light) = {
            use esp_idf_hal::{
                gpio::PinDriver,
                spi::{Dma, SpiConfig, SpiDeviceDriver, SpiDriverConfig},
                units::Hertz,
            };

            let remainder = Remainder {
                gpio22: peripherals.pins.gpio22,
                gpio27: peripherals.pins.gpio27,
                gpio35: peripherals.pins.gpio35,
                i2c0: peripherals.i2c0,
                i2c1: peripherals.i2c1,
                spi3: peripherals.spi3,
                rmt: peripherals.rmt,
            };

            let spi = SpiDeviceDriver::new_single(
                peripherals.spi2,              //SPI
                peripherals.pins.gpio14,       //SCLK
                peripherals.pins.gpio13,       //SDO / MISO
                Some(peripherals.pins.gpio12), //SDI / MOSI
                Some(peripherals.pins.gpio15), //CS / SS
                &SpiDriverConfig::new().dma(Dma::Auto(4096)),
                &SpiConfig::new().baudrate(Hertz(20_000_000)),
            )?;

            // GPIO
            let dc = PinDriver::output(peripherals.pins.gpio2)?;
            // let reset = PinDriver::output(peripherals.pins.gpio4)?;
            let back_light: PinDriver<'static, esp_idf_hal::gpio::Output> =
                PinDriver::output(peripherals.pins.gpio21)?;

            (remainder, spi, dc, back_light)
        };

        #[cfg(feature = "board-wave-esp32c6147")]
        let (remainder, spi, dc, back_light, reset) = {
            use embedded_graphics::prelude::Size;
            use esp_idf_hal::{
                delay::Ets,
                gpio::PinDriver,
                spi::{Dma, SpiConfig, SpiDeviceDriver, SpiDriverConfig},
                units::Hertz,
            };

            let remainder = Remainder {
                gpio0: peripherals.pins.gpio0,
                gpio1: peripherals.pins.gpio1,
                gpio2: peripherals.pins.gpio2,
                gpio3: peripherals.pins.gpio3,
                gpio4: peripherals.pins.gpio4,
                gpio5: peripherals.pins.gpio5,
                gpio9: peripherals.pins.gpio9,
                gpio12: peripherals.pins.gpio12,
                gpio13: peripherals.pins.gpio13,
                gpio18: peripherals.pins.gpio18,
                gpio19: peripherals.pins.gpio19,
                gpio20: peripherals.pins.gpio20,
                gpio23: peripherals.pins.gpio23,
                i2c0: peripherals.i2c0,
                rmt: peripherals.rmt,
            };

            let back_light = PinDriver::output(peripherals.pins.gpio22)?;

            let spi = SpiDeviceDriver::new_single(
                peripherals.spi2,
                peripherals.pins.gpio7,                 // SCLK
                peripherals.pins.gpio6,                 // MOSI
                None::<esp_idf_hal::gpio::AnyInputPin>, // MISO not used
                Some(peripherals.pins.gpio14),          // CS
                &SpiDriverConfig::new().dma(Dma::Auto(4096)),
                &SpiConfig::new().baudrate(Hertz(40_000_000)),
            )?;

            // Control pins
            let dc = PinDriver::output(peripherals.pins.gpio15)?;
            let reset = PinDriver::output(peripherals.pins.gpio21)?;

            (remainder, spi, dc, back_light, reset)
        };

        #[cfg(feature = "board-wave-esp32c6touch147")]
        let (remainder, spi, dc, back_light, reset, i2c, touch_driver_factory) = {
            use esp_idf_hal::{
                gpio::PinDriver,
                i2c::{I2cConfig, I2cDriver},
                spi::{Dma, SpiConfig, SpiDeviceDriver, SpiDriverConfig},
                units::Hertz,
            };

            let remainder = Remainder {
                gpio4: peripherals.pins.gpio4,
                gpio5: peripherals.pins.gpio5,
                gpio6: peripherals.pins.gpio6,
                gpio7: peripherals.pins.gpio7,
                gpio8: peripherals.pins.gpio8,
                gpio9: peripherals.pins.gpio9,
                // gpio18: peripherals.pins.gpio18,
                // gpio19: peripherals.pins.gpio19,
                // i2c0: peripherals.i2c0,
                rmt: peripherals.rmt,
            };

            let back_light = PinDriver::output(peripherals.pins.gpio23)?;

            let spi = SpiDeviceDriver::new_single(
                peripherals.spi2,
                peripherals.pins.gpio1,        // SCLK
                peripherals.pins.gpio2,        // MOSI
                Some(peripherals.pins.gpio3),  // MISO
                Some(peripherals.pins.gpio14), // CS
                &SpiDriverConfig::new().dma(Dma::Auto(4096)),
                &SpiConfig::new().baudrate(Hertz(40_000_000)),
            )?;

            // Control pins
            let dc = PinDriver::output(peripherals.pins.gpio15)?;
            let reset = PinDriver::output(peripherals.pins.gpio22)?;
            let sda = peripherals.pins.gpio18;
            let scl = peripherals.pins.gpio19;

            let config = I2cConfig::new().baudrate(Hertz(400_000));

            let i2c = Arc::new(Mutex::new(I2cDriver::new(
                peripherals.i2c0,
                sda,
                scl,
                &config,
            )?));

            let rotation: u16 = 0;
            let touch_driver_factory =
                TouchDriver::factory(peripherals.pins.gpio21, peripherals.pins.gpio20, &i2c)?;

            // int_pin, rst_pin, i2c, width, height, rotation): esp_idf_hal::gpio::AnyInputPin<'static> = peripherals.pins.gpio21.into();

            (
                remainder,
                spi,
                dc,
                back_light,
                reset,
                i2c,
                touch_driver_factory,
            )
        };

        #[cfg(feature = "board-devkitv1")]
        let remainder = Remainder {
            gpio4: peripherals.pins.gpio4,
            gpio13: peripherals.pins.gpio13,
            gpio14: peripherals.pins.gpio14,
            gpio16: peripherals.pins.gpio16,
            gpio17: peripherals.pins.gpio17,
            gpio18: peripherals.pins.gpio18,
            gpio19: peripherals.pins.gpio19,
            gpio21: peripherals.pins.gpio21,
            gpio22: peripherals.pins.gpio22,
            gpio27: peripherals.pins.gpio27,
            gpio23: peripherals.pins.gpio23,
            gpio25: peripherals.pins.gpio25,
            gpio26: peripherals.pins.gpio26,
            gpio32: peripherals.pins.gpio32,
            gpio33: peripherals.pins.gpio33,
            i2c0: peripherals.i2c0,
            i2c1: peripherals.i2c1,
            spi3: peripherals.spi3,
            rmt: peripherals.rmt,
        };

        #[cfg(feature = "board-xiao-esp32c6")]
        let remainder = {
            let remainder = Remainder {
                gpio2: peripherals.pins.gpio2,
                gpio21: peripherals.pins.gpio21,
                i2c0: peripherals.i2c0,
                rmt: peripherals.rmt,
            };

            remainder
        };

        #[cfg(feature = "board-supermini-esp32c3")]
        let remainder = {
            use esp_idf_hal::{
                gpio::AnyIOPin,
                spi::{Dma, SpiConfig, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
                units::Hertz,
            };

            let remainder = Remainder {
                gpio0: peripherals.pins.gpio0,
                gpio1: peripherals.pins.gpio1,
                gpio3: peripherals.pins.gpio3,
                gpio_sclk: peripherals.pins.gpio4,
                gpio_sdo_miso: peripherals.pins.gpio5,
                gpio_sdi_mosi: peripherals.pins.gpio6,
                gpio_cs_ss: peripherals.pins.gpio7,
                gpio10: peripherals.pins.gpio10,
                gpio20: peripherals.pins.gpio20,
                gpio21: peripherals.pins.gpio21,
                i2c0: peripherals.i2c0,
                rmt: peripherals.rmt,
                spi2: peripherals.spi2,
            };

            remainder
        };

        let builder = Self {
            nvs_partition,
            // failure_reason,
            problem_manager,
            features: Vec::new(),
            initializer,
            config_manager_builder,
            ap_mode,
            core_feature,
            core_feature_name,
            core_config_valid,
            core_feature_config,
            wifi_sender,
            modem,
            #[cfg(feature = "display")]
            orientation: DisplayOrientation::Rotate0,

            #[cfg(feature = "led")]
            led_manager,

            #[cfg(feature = "mipi-dsi-display")]
            spi,
            #[cfg(feature = "mipi-dsi-display")]
            dc,
            #[cfg(feature = "mipi-dsi-display")]
            back_light,
            #[cfg(feature = "mipi-dsi-display-reset")]
            reset,
            #[cfg(feature = "i2c")]
            i2c,
            #[cfg(feature = "touch-driver")]
            touch_driver_factory: touch_driver_factory,
        };
        Ok((builder, remainder))
    }

    #[cfg(feature = "mipi-dsi-display")]
    pub fn with_display_orientation(
        mut self,
        orientation: DisplayOrientation,
    ) -> anyhow::Result<Self> {
        self.orientation = orientation;

        Ok(self)
    }

    pub fn with_feature(mut self, feature: Box<dyn DynFeature>) -> anyhow::Result<Self> {
        self.internal_add_feature(feature, false)?;
        Ok(self)
    }

    fn internal_add_feature(
        &mut self,
        feature: Box<dyn DynFeature>,
        internal: bool,
    ) -> anyhow::Result<()> {
        // let descriptor = feature.init(&mut self.initializer)?;
        // self.features.push(feature);

        let descriptor = feature.do_init(&mut self.initializer)?;
        let name = descriptor.name.clone();
        let (config, _valid) = self
            .config_manager_builder
            .add_feature(descriptor, internal)?;
        self.features.push(FeatureHolder {
            feature,
            config,
            name,
        });

        Ok(())
    }

    pub fn build(mut self) -> anyhow::Result<Esp32PlatformRunner> {
        self.features.shrink_to_fit();

        #[cfg(feature = "display")]
        let mut display_manager;
        #[cfg(feature = "imu")]
        let mut imu;
        #[cfg(feature = "touch-driver")]
        let mut touch_driver;

        #[cfg(feature = "board-cyd")]
        {
            use embedded_graphics::prelude::Size;
            use esp_idf_hal::{
                delay::Ets,
                gpio::PinDriver,
                spi::{Dma, SpiConfig, SpiDeviceDriver, SpiDriverConfig},
                units::Hertz,
            };
            // SPI
            // let spi = SpiDeviceDriver::new_single(
            //     peripherals.spi2,
            //     peripherals.pins.gpio14,
            //     peripherals.pins.gpio13,
            //     Some(peripherals.pins.gpio12),
            //     Some(peripherals.pins.gpio15),
            //     &SpiDriverConfig::new().dma(Dma::Auto(4096)),
            //     &SpiConfig::new()
            //         .baudrate(Hertz(20_000_000))
            //         ,
            // )?;

            // // GPIO
            // let dc = PinDriver::output(peripherals.pins.gpio2)?;
            // // let reset = PinDriver::output(peripherals.pins.gpio4)?;
            // let mut back_light: PinDriver<'static, esp_idf_hal::gpio::Output> = PinDriver::output(peripherals.pins.gpio21)?;

            let mut orientation = mipidsi::options::Orientation::new().flip_horizontal();

            match self.orientation {
                DisplayOrientation::Rotate0 => {}
                DisplayOrientation::Rotate90 => {
                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg90);
                }
                DisplayOrientation::Rotate180 => {
                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg180);
                }
                DisplayOrientation::Rotate270 => {
                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg270);
                }
            };

            let di = mipi_dsi_display_manager::EspDi {
                spi: self.spi,
                dc: self.dc,
                xoffset: 0,
                yoffset: 0,
            };
            let mut delay = Ets;
            let display = match mipidsi::Builder::new(mipidsi::models::ILI9341Rgb565, di)
                // .reset_pin(reset)
                .display_size(240, 320)
                .orientation(orientation)
                .color_order(mipidsi::options::ColorOrder::Bgr)
                .init(&mut delay)
            {
                Ok(d) => d,
                Err(e) => anyhow::bail!("Display init error {:?}", e),
            };

            // enable back_light
            self.back_light.set_high()?;

            display_manager = mipi_dsi_display_manager::MipiDsiDisplayManager {
                back_light: self.back_light,
                display,
            };
        }

        #[cfg(feature = "board-wave-esp32c6147")]
        {
            use embedded_graphics::prelude::Size;
            use esp_idf_hal::{
                delay::Ets,
                gpio::PinDriver,
                spi::{Dma, SpiConfig, SpiDeviceDriver, SpiDriverConfig},
                units::Hertz,
            };

            let mut orientation = mipidsi::options::Orientation::new();
            let xoffset;
            let yoffset;

            match self.orientation {
                DisplayOrientation::Rotate0 => {
                    xoffset = 34;
                    yoffset = 0;
                }
                DisplayOrientation::Rotate90 => {
                    xoffset = 0;
                    yoffset = -34;

                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg90);
                }
                DisplayOrientation::Rotate180 => {
                    xoffset = -34;
                    yoffset = 0;

                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg180);
                }
                DisplayOrientation::Rotate270 => {
                    xoffset = 0;
                    yoffset = 34;

                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg270);
                }
            };

            let di = mipi_dsi_display_manager::EspDi {
                spi: self.spi,
                dc: self.dc,
                xoffset,
                yoffset,
            };
            let mut delay = Ets;

            let display = match mipidsi::Builder::new(mipidsi::models::ST7789, di)
                .reset_pin(self.reset)
                .display_size(172, 320)
                .orientation(orientation)
                .color_order(mipidsi::options::ColorOrder::Rgb)
                .invert_colors(mipidsi::options::ColorInversion::Inverted)
                .init(&mut delay)
            {
                Ok(d) => d,
                Err(e) => anyhow::bail!("Display init error {:?}", e),
            };

            // enable back_light
            self.back_light.set_high()?;

            display_manager = mipi_dsi_display_manager::MipiDsiDisplayManager {
                back_light: self.back_light,
                display,
            };
        }

        #[cfg(feature = "board-wave-esp32c6touch147")]
        {
            use crate::ahrs::qmi8658::Qmi8658;
            use esp_idf_hal::delay::Ets;

            let mut orientation = mipidsi::options::Orientation::new().flip_horizontal();
            let xoffset;
            let yoffset;

            match self.orientation {
                DisplayOrientation::Rotate0 => {
                    xoffset = -34;
                    yoffset = 0;
                }
                DisplayOrientation::Rotate90 => {
                    xoffset = 0;
                    yoffset = 34;

                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg90);
                }
                DisplayOrientation::Rotate180 => {
                    xoffset = 34;
                    yoffset = 0;

                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg180);
                }
                DisplayOrientation::Rotate270 => {
                    xoffset = 0;
                    yoffset = -34;

                    orientation = orientation.rotate(mipidsi::options::Rotation::Deg270);
                }
            };

            let di = mipi_dsi_display_manager::EspDi {
                spi: self.spi,
                dc: self.dc,
                xoffset,
                yoffset,
            };
            let mut delay = Ets;

            let display = match mipidsi::Builder::new(mipidsi::models::ST7789, di)
                .reset_pin(self.reset)
                .display_size(172, 320)
                .orientation(orientation)
                .color_order(mipidsi::options::ColorOrder::Bgr)
                .init(&mut delay)
            {
                Ok(d) => d,
                Err(e) => anyhow::bail!("Display init error {:?}", e),
            };

            // enable back_light
            self.back_light.set_high()?;

            display_manager = mipi_dsi_display_manager::MipiDsiDisplayManager {
                back_light: self.back_light,
                display,
            };

            touch_driver = self
                .touch_driver_factory
                .build(172, 320, self.orientation)?;

            imu = Qmi8658::new(&self.i2c)?;

            touch_driver.init()?;

            touch_driver.start_touch_task()?;

            // crate::touch::axs5106l::start_touch_task(
            //     self.touch_pin,
            //     &self.i2c,
            //     172, 320,
            //     rotation
            // )?;
        }

        #[cfg(feature = "display")]
        display_manager.set_status(&Status::Initializing(InitStatus::Starting))?;

        let sys_loop = EspSystemEventLoop::take()?;
        // let timer_service = EspTaskTimerService::new()?;

        let wifi_manager = WiFiManager::new(
            self.modem,
            sys_loop,
            self.nvs_partition.clone(),
            &self.problem_manager,
            self.wifi_sender,
        )?;

        let bare_config_manager = self.config_manager_builder.build();

        let mut server_manager = EspHttpServerManager::new()?;

        server_manager.init_common_pages()?;
        server_manager.init_captive_portal(&self.ap_mode)?;

        let config_manager = Arc::new(bare_config_manager);
        ConfigManager::create_pages(&config_manager, &mut server_manager)?;

        // This should be in the app

        let cloned_ap_mode = self.ap_mode.clone();
        server_manager.on("/", Method::Get, move |req| {
            // info!("Received request for / from {}", req.connection().remote_addr());

            info!("Received {:?} request for {}", req.method(), req.uri());

            if cloned_ap_mode.lock().unwrap().clone() {
                req.into_response(302, Some("Found"), &[("Location", "/config")])?;
            } else {
                let mut resp = req.into_ok_response()?;
                resp.write(
                    r#"
                    <!DOCTYPE html>
                    <html lang="en">
                    <head>
                        <meta charset="utf-8" />
                        <meta name="viewport" content="width=device-width, initial-scale=1" />
                        <title>ESP32 Home</title>
                        <link rel="stylesheet" href="/main.css">
                    </head>
                    <body>
                        <div class="page">
                            <h1>ESP32 Home</h1>
                            <p>Welcome to the ESP32 home page!</p>
                            <p>Current time: "#
                        .as_bytes(),
                )?;

                let now = Local::now();
                let time = now.format("%Y-%m-%d %H:%M:%S").to_string();
                resp.write(time.as_bytes())?;
                resp.write(
                    r#"</p>
                        </div>
                    </body>
                    </html>
                    "#
                    .as_bytes(),
                )?;
            }
            Ok(())
        })?;

        // END APP CODE

        Ok(Esp32PlatformRunner {
            sparko_std: Esp32Platform {
                wifi_manager,
                #[cfg(feature = "led")]
                led_manager: self.led_manager,
                config_manager,
                server_manager,
                features: self.features,
                ap_mode: self.ap_mode,
                core_config_valid: self.core_config_valid,
                #[cfg(feature = "display")]
                display_manager: Arc::new(Mutex::new(display_manager)),
                #[cfg(feature = "i2c")]
                i2c: self.i2c,
                #[cfg(feature = "imu")]
                imu_manager: crate::ahrs::ImuManager::from_imu(imu),
                #[cfg(feature = "touch-driver")]
                touch_driver,
            },
            initializer: self.initializer,
            core_feature_holder: FeatureHolder {
                feature: Box::new(self.core_feature),
                config: self.core_feature_config,
                name: self.core_feature_name,
            },
        })
    }
}

struct FeatureHolder {
    feature: Box<dyn DynFeature>,
    config: Config,
    name: String,
}

pub struct Esp32PlatformRunner {
    pub sparko_std: Esp32Platform,
    initializer: Esp32PlatformInitializer,
    core_feature_holder: FeatureHolder,
}

impl Esp32PlatformRunner {
    pub fn start(mut self) -> anyhow::Result<()> {
        self.sparko_std
            .start(self.initializer, self.core_feature_holder)
    }
}

pub struct Esp32Platform {
    pub wifi_manager: WiFiManager<'static>,
    #[cfg(feature = "rgb-led")]
    pub led_manager: RgbLedManager<'static>,
    #[cfg(feature = "mono-led")]
    pub led_manager: MonoLedManager,
    #[cfg(feature = "simple-led")]
    pub led_manager: SimpleLedManager<'static>,
    pub config_manager: Arc<ConfigManager>,
    pub server_manager: EspHttpServerManager<'static>,
    features: Vec<FeatureHolder>,
    pub ap_mode: Arc<Mutex<bool>>,
    core_config_valid: bool,

    #[cfg(feature = "mipi-dsi-display")]
    pub display_manager: Arc<Mutex<mipi_dsi_display_manager::MipiDsiDisplayManager>>,
    #[cfg(feature = "i2c")]
    i2c: Arc<Mutex<esp_idf_hal::i2c::I2cDriver<'static>>>,
    #[cfg(feature = "qmi8658")]
    pub imu_manager: crate::ahrs::ImuManager,
    #[cfg(feature = "touch-driver")]
    pub touch_driver: TouchDriver,
}

impl Platform for Esp32Platform {}

impl Esp32Platform {
    pub fn builder() -> anyhow::Result<(Esp32PlatformBuilder, Remainder)> {
        Esp32PlatformBuilder::new()
    }

    pub fn set_status(&mut self, status: Status) -> anyhow::Result<()> {
        #[cfg(feature = "led")]
        self.led_manager.set_status(&status)?;
        #[cfg(feature = "display")]
        self.display_manager.lock().unwrap().set_status(&status)?;

        Ok(())
    }

    fn start_feature(
        &mut self,
        mut feature_holder: FeatureHolder,
        mut initializer: &mut Esp32PlatformInitializer,
    ) {
        if feature_holder.config.enabled.is_enabled() {
            match feature_holder.feature.start_with_config(
                self,
                &mut initializer,
                &feature_holder.config.spec,
            ) {
                Ok(_) => info!("Started Feature {}", feature_holder.name),
                Err(error) => error!("FAILED to start Feature {}: {}", feature_holder.name, error),
            }
        } else {
            info!("Feature {} is disabled", feature_holder.name)
        }

        self.features.push(feature_holder);
    }

    fn start_client(
        &mut self,
        mut initializer: Esp32PlatformInitializer,
        core_feature_holder: FeatureHolder,
    ) -> anyhow::Result<()> {
        // start wifi

        self.set_status(Status::Initializing(InitStatus::AwaitingClientIpAddress))?;
        let ip_address = self
            .wifi_manager
            .start_client(&core_feature_holder.config)?;

        self.set_status(Status::Initializing(InitStatus::AwaitingTimeSync))?;
        info!("Wifi started: ip_address={}", &ip_address);

        let sntp = EspSntp::new_default()?;

        info!("SNTP started, waiting for time sync...");

        loop {
            if let SyncStatus::Completed = sntp.get_sync_status() {
                break;
            }
            info!("still waiting for time sync...");
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        self.set_status(Status::Initializing(InitStatus::StartingFeatures))?;

        let datetime: chrono::DateTime<Local> = Local::now();
        info!("Time synced: {}", datetime.format("%Y-%m-%d %H:%M:%S"));

        let features = std::mem::take(&mut self.features);
        self.features = Vec::with_capacity(features.len() + 1);

        self.start_feature(core_feature_holder, &mut initializer);

        for feature_holder in features {
            self.start_feature(feature_holder, &mut initializer);
        }

        let mut task_manager = initializer.task_manager_builder.build();

        self.set_status(Status::Running)?;

        // This should never return
        task_manager.run(self)
    }

    fn start(
        &mut self,
        initializer: Esp32PlatformInitializer,
        core_feature_holder: FeatureHolder,
    ) -> anyhow::Result<()> {
        log::info!("sparko_cyd: top of run");
        if self.core_config_valid {
            log::info!("Loaded config");

            if let Err(error) = self.start_client(initializer, core_feature_holder) {
                log::error!("Error starting client: {}", error);
                self.set_status(Status::Error)?;
            } else {
                log::info!("Client mode started successfully");
                return Ok(());
            }
        } else {
            self.set_status(Status::Setup)?;
            info!("Invalid config, starting AP mode");
        }

        *self.ap_mode.lock().unwrap() = true;

        let server_addr = self.wifi_manager.start_access_point()?;

        thread::spawn(move || Self::captive_dns_server(server_addr));

        loop {
            log::info!("Top of AP loop");
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
        // Self::system_halt("AP Loop terminated");
    }

    // fn system_halt<S: AsRef<str>>(s: S) {
    //     // TODO: Implement BSOD or similar system halt mechanism here
    //     println!("{}", s.as_ref());

    //     let bt = Backtrace::force_capture();
    //     println!("Stack trace:\n{bt}");

    //     std::process::exit(1);
    // }

    fn captive_dns_server(server_addr: std::net::Ipv4Addr) {
        info!("DNS server start");
        let socket = UdpSocket::bind("0.0.0.0:53").unwrap();
        let addr_bytes = server_addr.octets();
        loop {
            let mut buf = [0u8; 512];

            // info!("DNS server recv_from...");
            let (size, src) = socket.recv_from(&mut buf).unwrap();

            // info!("DNS server recv_from...{:?}", &buf[..size]);

            let response = Self::build_dns_response(&buf[..size], &addr_bytes);

            socket.send_to(&response, src).unwrap();
        }
    }

    fn build_dns_response(query: &[u8], server_addr: &[u8; 4]) -> Vec<u8> {
        // info!("Received DNS query: {:?}", query);
        let mut resp = query.to_vec();

        resp[2] |= 0x80; // set QR bit (response)
        resp[3] |= 0x80; // set RD bit (recursion desired, optional)

        // Set ANCOUNT to 1 (answer count)
        resp[6] = 0x00;
        resp[7] = 0x01;

        resp.extend_from_slice(&[
            0xc0,
            0x0c, // pointer to domain
            0x00,
            0x01, // type A
            0x00,
            0x01, // class IN
            0x00,
            0x00,
            0x00,
            0x3c, // TTL (60 seconds)
            0x00,
            0x04, // data length (4 bytes for IPv4)
            server_addr[0],
            server_addr[1],
            server_addr[2],
            server_addr[3], // IP address
        ]);

        // info!("Sending DNS response: {:?}", resp);

        resp
    }
}
