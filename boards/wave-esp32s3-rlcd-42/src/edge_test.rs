//! A test pattern, to find any pixels at the display's edges which cannot be seen: three frames, a pixel wide, at 0, 2
//! and 4 pixels in from the edges, so that each edge shows three lines if all its pixels can be seen, and the edges'
//! names, and the size, in the middle.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};
use sparko_embedded_std::config::{ConfigSpec, FeatureConfig};
use sparko_embedded_std::feature::FeatureDescriptor;
use sparko_embedded_std::graphics::DisplayManager;
use sparko_embedded_std::platform::PlatformInitializer;
use sparko_embedded_std::task::scheduler::ScheduledTask;
use sparko_esp_idf::{BoardDisplayManager, Esp32Platform, Esp32PlatformInitializer, Feature};

#[derive(FeatureConfig)]
#[config(label = "Edge Test")]
pub struct EdgeTestConfig {}

pub struct EdgeTest;

impl Feature for EdgeTest {
    type Config = EdgeTestConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor { name: "EdgeTest".to_string(), config: EdgeTestConfig::to_config_spec()? })
    }

    fn start(&mut self, sparko: &mut Esp32Platform, initializer: &mut Esp32PlatformInitializer, _config: EdgeTestConfig) -> anyhow::Result<()> {
        initializer.add_task(Box::new(DrawTask { display_manager: sparko.display_manager.clone(), drawn: None }), "* * * * * *")?;
        Ok(())
    }
}

/// Draws the pattern whenever the screen has been cleared, e.g. when the platform has started, see
/// DisplayManager::generation.
struct DrawTask {
    display_manager: std::sync::Arc<std::sync::Mutex<BoardDisplayManager>>,
    drawn: Option<u32>,
}

impl ScheduledTask<Esp32Platform> for DrawTask {
    fn name(&self) -> &str {
        "Edge Test"
    }

    fn run(&mut self, _sparko: &mut Esp32Platform) -> anyhow::Result<()> {
        let mut display_manager = self.display_manager.lock().unwrap();
        if display_manager.overlay_active() || self.drawn == Some(display_manager.generation()) {
            return Ok(());
        }
        self.drawn = Some(display_manager.generation());
        let display = display_manager.display();
        let size = display.bounding_box().size;
        let ink = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
        for inset in [0, 2, 4] {
            Rectangle::new(Point::new(inset, inset), Size::new(size.width - 2 * inset as u32, size.height - 2 * inset as u32))
                .into_styled(ink)
                .draw(display)?;
        }
        let text = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let (width, height) = (size.width as i32, size.height as i32);
        Text::with_alignment("TOP", Point::new(width / 2, 26), text, Alignment::Center).draw(display)?;
        Text::with_alignment("BOTTOM", Point::new(width / 2, height - 12), text, Alignment::Center).draw(display)?;
        Text::with_alignment("LEFT", Point::new(12, height / 2), text, Alignment::Left).draw(display)?;
        Text::with_alignment("RIGHT", Point::new(width - 12, height / 2), text, Alignment::Right).draw(display)?;
        Text::with_alignment(&format!("{} x {}", size.width, size.height), Point::new(width / 2, height / 2 - 20), text, Alignment::Center)
            .draw(display)?;
        Text::with_alignment("3 lines on each edge:", Point::new(width / 2, height / 2 + 10), text, Alignment::Center).draw(display)?;
        Text::with_alignment("0, 2 and 4 px in", Point::new(width / 2, height / 2 + 32), text, Alignment::Center).draw(display)?;
        Ok(())
    }
}
