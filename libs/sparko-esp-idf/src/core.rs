use std::net::Ipv4Addr;
use std::sync::mpsc::Receiver;

use chrono::Local;
use log::info;
use sparko_embedded_std::{
    config::{Config, ConfigSpec, ConfigSpecValue, TypedValue},
    feature::FeatureDescriptor,
    platform::Platform,
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
pub const SSID: &str = "ssid";
pub const WIFI_PASSWORD: &str = "wifi_password";
pub const MDNS_HOSTNAME: &str = "mdns_hostname";
pub const TIMEZONE: &str = "time_zone";

pub const SSID_LEN: usize = 32;
pub const PASSWORD_LEN: usize = 64;
pub const HOSTNAME_LEN: usize = 32;

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
    fn init(
        &self,
        _init: &mut crate::Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        let config = ConfigSpec::builder()
            .with(
                SSID.to_string(),
                ConfigSpecValue::new(TypedValue::String(SSID_LEN, None), true),
            )?
            .with(
                WIFI_PASSWORD.to_string(),
                ConfigSpecValue::new(TypedValue::String(PASSWORD_LEN, None), true),
            )?
            .with(
                MDNS_HOSTNAME.to_string(),
                ConfigSpecValue::new(TypedValue::String(HOSTNAME_LEN, None), true),
            )?
            .with(
                TIMEZONE.to_string(),
                ConfigSpecValue::new(TypedValue::TimeZone(Some(TimeZone::Utc)), true),
            )?
            .build();

        Ok(FeatureDescriptor {
            name: CORE_FEATURE_NAME.to_string(),
            config,
        })
    }

    fn start(
        &mut self,
        _sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &Config,
    ) -> anyhow::Result<()> {
        let opt_config = config.map.get(TIMEZONE);
        if let Some(config) = opt_config {
            if let TypedValue::TimeZone(Some(tz)) = config {
                Self::set_as_system_timezone(&tz);
            } else {
                anyhow::bail!("Timezone config value has wrong type");
            }
        } else {
            Self::set_as_system_timezone(&TimeZone::Utc);
        };

        let local_time = Local::now();
        info!("Local time is: {}", local_time.format("%Y-%m-%d %H:%M:%S"));

        let hostname = config.get_valid(MDNS_HOSTNAME)?;

        self.mdns_responder.start(hostname)?;

        let resolve_task = ResolveTask::new(config)?;
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

impl ResolveTask {
    pub fn new(_config: &Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
