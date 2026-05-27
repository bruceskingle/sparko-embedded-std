use crate::Esp32Platform;
use crate::Esp32PlatformInitializer;
use crate::{Feature, FeatureDescriptor};
use embedded_graphics::prelude::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;
use esp_idf_svc::http::Method;
use esp_idf_svc::http::client::EspHttpConnection;
use log::info;
use rgb::RGB8;
use sparko_embedded_std::Layout;
use sparko_embedded_std::config::Config;
use sparko_embedded_std::config::ConfigSpec;
use sparko_embedded_std::config::ConfigSpecValue;
use sparko_embedded_std::config::TypedValue;
use sparko_embedded_std::config::parse_rgb8;
use sparko_embedded_std::graphics::ClockRenderer;
use sparko_embedded_std::graphics::DisplayManager;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;
use std::net::IpAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

//                                          123456789012345<-------- Max Name Length 15
pub const CLOCK_COLOR: &str = "clock_color";
pub const BG_COLOR: &str = "bg_color";

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
            .with(
                CLOCK_COLOR.to_string(),
                ConfigSpecValue::new(TypedValue::Color(Some(parse_rgb8("#00ff00")?)), true),
            )?
            .with(
                BG_COLOR.to_string(),
                ConfigSpecValue::new(TypedValue::Color(Some(parse_rgb8("#000000")?)), true),
            )?
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
        let clock_color = match config.map.get(CLOCK_COLOR) {
            Some(color) => {
                if let TypedValue::Color(Some(val)) = color {
                    val
                } else {
                    &RGB8 { r: 0, g: 255, b: 0 }
                }
            }
            None => &RGB8 { r: 0, g: 255, b: 0 },
        };

        let bg_color = match config.map.get(BG_COLOR) {
            Some(color) => {
                if let TypedValue::Color(Some(val)) = color {
                    val
                } else {
                    &RGB8 { r: 0, g: 0, b: 0 }
                }
            }
            None => &RGB8 { r: 0, g: 0, b: 0 },
        };
        initializer.add_task(
            Box::new(ResolveTask {
                clock_renderer: ClockRenderer::new(
                    &sparko.display_manager,
                    self.layout,
                    clock_color,
                    bg_color,
                )?,
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
    // fn run(&mut self, _sparko_cyd: &dyn Esp32Platform) -> anyhow::Result<()> {
    //     let clock_renderer =
    // }

    fn name(&self) -> &str {
        "Analog Clock"
    }

    fn run(&mut self, sparko_embedded: &mut Esp32Platform) -> anyhow::Result<()> {
        self.clock_renderer.update()
    }
}
