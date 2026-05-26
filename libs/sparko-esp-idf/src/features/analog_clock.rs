use crate::Esp32Platform;
use crate::Esp32PlatformInitializer;
use crate::{Feature, FeatureDescriptor};
use embedded_graphics::prelude::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;
use esp_idf_svc::http::Method;
use esp_idf_svc::http::client::EspHttpConnection;
use log::info;
use sparko_embedded_std::Layout;
use sparko_embedded_std::config::Config;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config::ConfigSpecValue;
use sparko_embedded_std::config::TypedValue;
use sparko_embedded_std::graphics::ClockRenderer;
use sparko_embedded_std::graphics::DisplayManager;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;
use std::net::IpAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

//                                           123456789012345<-------- Max Name Length 15
// pub const USER_NAME: &str =                 "user_name";
// pub const PASSWORD: &str =                  "password";
// pub const HOSTNAME: &str =                  "hostname";
// pub const BASE_SERVICE_URL: &str =          "base_url";
// pub const GET_IP_URL: &str =                "get_ip_url";
// pub const GET_REQUIRES_STRIP: &str =        "get_req_strip";
// pub const UPDATE_URL: &str =                "update_url";
// pub const UPDATE_REQUIRES_ADDRESS: &str =   "upd_req_addr";
// pub const UPDATE_INTERVAL: &str =           "upd_int";
// pub const SCHEDULE: &str =                  "schedule";

pub struct AnalogClockBuilder {
    layout: Option<Layout>,
}

impl AnalogClockBuilder {
    pub fn new() -> Self {
        Self { layout: None }
    }

    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = Some(layout);
        self
    }

    pub fn build(self) -> anyhow::Result<AnalogClock> {
        Ok(AnalogClock {
            layout: self.layout.unwrap_or(|bounding_box: &Rectangle| {
                Rectangle::new(
                    Point::new(bounding_box.top_left.x + 1, bounding_box.top_left.y + 1),
                    Size::new(bounding_box.size.width - 2, bounding_box.size.height - 2),
                )
            }),
        })
    }
}

pub struct AnalogClock {
    layout: Layout,
}

impl AnalogClock {
    pub fn builder() -> AnalogClockBuilder {
        AnalogClockBuilder::new()
    }
}

impl Feature for AnalogClock {
    fn init(
        &self,
        _initializer: &mut Esp32PlatformInitializer,
    ) -> anyhow::Result<FeatureDescriptor> {
        info!("AnalogClock::init()");
        let config = ConfigSpec::builder()
            // .with(USER_NAME.to_string(), ConfigSpecValue::new(TypedValue::String(32, None), true))?
            // .with(PASSWORD.to_string(), ConfigSpecValue::new(TypedValue::String(32, None), true))?
            // .with(HOSTNAME.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // // .with(BASE_SERVICE_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // .with(GET_IP_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // // .with(GET_REQUIRES_STRIP.to_string(), ConfigSpecValue::new(TypedValue::Bool(false), false))?
            // .with(UPDATE_URL.to_string(), ConfigSpecValue::new(TypedValue::String(64, None), true))?
            // .with(UPDATE_REQUIRES_ADDRESS.to_string(), ConfigSpecValue::new(TypedValue::Bool(false), false ))?
            // .with(SCHEDULE.to_string(), ConfigSpecValue::new(TypedValue::Cron(None), true))?
            .build();

        Ok(FeatureDescriptor {
            name: "AnalogClock".to_string(),
            config,
        })
    }

    fn start(
        &mut self,
        sparko: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &Config,
    ) -> anyhow::Result<()> {
        initializer.add_task(
            Box::new(ResolveTask {
                clock_renderer: ClockRenderer::new(&sparko.display_manager, self.layout)?,
            }),
            "* * * * * *",
        )?;
        Ok(())
    }
}

pub struct ResolveTask<DM>
where
    DM: DisplayManager,
{
    clock_renderer: ClockRenderer<DM>,
}

impl<DM> ScheduledTask<Esp32Platform> for ResolveTask<DM>
where
    DM: DisplayManager,
{
    // fn run(&mut self, _sparko_cyd: &dyn SparkoEmbeddedStd) -> anyhow::Result<()> {
    //     let clock_renderer =
    // }

    fn name(&self) -> &str {
        "Analog Clock"
    }

    fn run(&mut self, sparko_embedded: &mut Esp32Platform) -> anyhow::Result<()> {
        self.clock_renderer.update()
    }
}
