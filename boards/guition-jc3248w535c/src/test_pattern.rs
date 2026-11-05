//! A test pattern, to see that the display is the right way up and its colours right: three frames, a pixel wide, at 0,
//! 2 and 4 pixels in from the edges, so that each edge shows three lines if all its pixels can be seen, the edges'
//! names, the size, and bars of red, green, blue and white, each named, so that swapped bytes or channels show.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
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
#[config(label = "Test Pattern")]
pub struct TestPatternConfig {}

pub struct TestPattern;

impl Feature for TestPattern {
    type Config = TestPatternConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor { name: "TestPattern".to_string(), config: TestPatternConfig::to_config_spec()? })
    }

    fn start(&mut self, sparko: &mut Esp32Platform, initializer: &mut Esp32PlatformInitializer, _config: TestPatternConfig) -> anyhow::Result<()> {
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
        "Test Pattern"
    }

    fn run(&mut self, _sparko: &mut Esp32Platform) -> anyhow::Result<()> {
        let mut display_manager = self.display_manager.lock().unwrap();
        if display_manager.overlay_active() || self.drawn == Some(display_manager.generation()) {
            return Ok(());
        }
        self.drawn = Some(display_manager.generation());
        let display = display_manager.display();
        let size = display.bounding_box().size;
        let (width, height) = (size.width as i32, size.height as i32);
        let bars = [("RED", Rgb565::RED), ("GREEN", Rgb565::GREEN), ("BLUE", Rgb565::BLUE), ("WHITE", Rgb565::WHITE)];
        let bar_width = (width - 80) / bars.len() as i32;
        for (index, (name, colour)) in bars.iter().enumerate() {
            let x = 40 + index as i32 * bar_width;
            Rectangle::new(Point::new(x, height - 110), Size::new(bar_width as u32 - 8, 50))
                .into_styled(PrimitiveStyle::with_fill(*colour))
                .draw(display)?;
            let text = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
            Text::with_alignment(name, Point::new(x + bar_width / 2 - 4, height - 40), text, Alignment::Center).draw(display)?;
        }
        let ink = PrimitiveStyle::with_stroke(Rgb565::WHITE, 1);
        for inset in [0, 2, 4] {
            Rectangle::new(Point::new(inset, inset), Size::new(size.width - 2 * inset as u32, size.height - 2 * inset as u32))
                .into_styled(ink)
                .draw(display)?;
        }
        let text = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        Text::with_alignment("TOP", Point::new(width / 2, 26), text, Alignment::Center).draw(display)?;
        Text::with_alignment("BOTTOM", Point::new(width / 2, height - 12), text, Alignment::Center).draw(display)?;
        Text::with_alignment("LEFT", Point::new(12, height / 2), text, Alignment::Left).draw(display)?;
        Text::with_alignment("RIGHT", Point::new(width - 12, height / 2), text, Alignment::Right).draw(display)?;
        Text::with_alignment(&format!("{} x {}", size.width, size.height), Point::new(width / 2, height / 2 - 60), text, Alignment::Center)
            .draw(display)?;
        Text::with_alignment("3 lines on each edge:", Point::new(width / 2, height / 2 - 30), text, Alignment::Center).draw(display)?;
        Text::with_alignment("0, 2 and 4 px in", Point::new(width / 2, height / 2 - 8), text, Alignment::Center).draw(display)?;
        Ok(())
    }
}
