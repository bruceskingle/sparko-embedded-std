use std::net::Ipv4Addr;
use std::sync::mpsc::Receiver;

use chrono::Local;
use log::info;
use sparko_embedded_std::config::FeatureConfig;
use sparko_embedded_std::{
    config::{ConfigSpec, ConfigSpecValue, TypedValue},
    feature::FeatureDescriptor,
    task::scheduler::ScheduledTask,
    tz::TimeZone,
};

use crate::mdns::MdnsResponder;
use crate::{
    Feature,
    esp32_platform::{Esp32Platform, Esp32PlatformInitializer},
};
use sparko_embedded_std::platform::PlatformInitializer;

pub const CORE_FEATURE_NAME: &str = "core";
pub const SSID_LEN: usize = 32;
pub const PASSWORD_LEN: usize = 64;

#[derive(FeatureConfig)]
pub struct CoreConfig {
    pub ssid: heapless::String<SSID_LEN>,
    pub wifi_password: heapless::String<PASSWORD_LEN>,
    #[config(len = 32)]
    pub mdns_hostname: String,
    pub time_zone: TimeZone,
}

pub struct Core {
    // The core feature provides wifi and mDNS
    mdns_responder: MdnsResponder,
}

impl Core {
    pub fn new(wifi_receiver: Receiver<Ipv4Addr>) -> anyhow::Result<Self> {
        Ok(Self {
            mdns_responder: MdnsResponder::new(wifi_receiver),
        })
    }

    fn set_as_system_timezone(time_zone: &TimeZone) {
        let tz = std::ffi::CString::new(time_zone.to_posix_tz()).unwrap();
        unsafe {
            esp_idf_sys::setenv(b"TZ\0".as_ptr() as *const u8, tz.as_ptr(), 1);
            esp_idf_sys::tzset();
        }
        log::info!(
            "System timezone set to {} ({})",
            time_zone.to_str(),
            time_zone.to_posix_tz()
        );
    }
}

impl Feature for Core {
    type Config = CoreConfig;

    fn init(
        &self,
        _init: &mut crate::Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor {
            name: CORE_FEATURE_NAME.to_string(),
            config: CoreConfig::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        _sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: CoreConfig,
    ) -> anyhow::Result<()> {
        Self::set_as_system_timezone(&config.time_zone);

        let local_time = Local::now();
        info!("Local time is: {}", local_time.format("%Y-%m-%d %H:%M:%S"));

        self.mdns_responder.start(config.mdns_hostname)?;

        let resolve_task = ResolveTask {};
        initializer.add_task(Box::new(resolve_task), "0 * * * * *")?;
        Ok(())
    }
}

pub struct ResolveTask {}

impl ScheduledTask<Esp32Platform> for ResolveTask {
    fn run(&mut self, _sparko_cyd: &mut Esp32Platform) -> anyhow::Result<()> {
        log::info!("Top of loop");

        let datetime = Local::now();
        info!("Time now: {}", datetime.format("%Y-%m-%d %H:%M:%S"));

        let heap_free = unsafe { esp_idf_sys::esp_get_free_heap_size() };
        let heap_min = unsafe { esp_idf_sys::esp_get_minimum_free_heap_size() };
        log::info!("heap free={} min={}", heap_free, heap_min);

        // TODO: force a reset if we run low on heap
        Ok(())
    }

    fn name(&self) -> &str {
        "Core"
    }
}
