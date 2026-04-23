//! A simple analog clock example using embedded-graphics.
//! Based on https://github.com/embedded-graphics/examples/blob/main/eg-0.8/examples/demo-analog-clock.rs

use core::f32::consts::PI;
use chrono::{Local, Timelike};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder},
};
use micromath::F32Ext;

use crate::graphics::{DisplayManager, DrawTargetResultExt};
const MARGIN: u32 = 10;


use embedded_graphics::prelude::DrawTarget;

pub struct ClockRenderer {
    clock_face: Circle,
    clock_color: super::Color,
    bg_color: super::Color,
    hour: u32,
    minute: u32,
    second: u32,
}

impl ClockRenderer
{
    pub fn new<'a, DM>(manager: &'a mut DM) -> anyhow::Result<Self> 
    where
        DM: DisplayManager,
    {
        let target: &mut DM::Display = manager.display();
        let clock_face = Self::create_face(target);
        let mut clock_renderer = ClockRenderer {
            clock_face,
            clock_color: super::Color::Green,
            bg_color: super::Color::Black,
            hour: 0,
            minute: 0,
            second: 0,
        };

        clock_renderer.draw(manager)?;

        Ok(clock_renderer)
    }

    pub fn draw<'a, DM>(&mut self, manager: &'a mut DM)  -> anyhow::Result<()>
    where
        DM: DisplayManager,
    {
        manager.fill_color(super::Color::Black)?;
        let clock_color = manager.map_color(&self.clock_color);
        let bg_color = manager.map_color(&self.bg_color);

        let now = Local::now();

        let hour = now.hour();
        let minute = now.minute();
        let second = now.second();


        Self::draw_face(manager.display(), &self.clock_face, clock_color).anyhow()?;

        Self::draw_hand(manager.display(), &self.clock_face, clock_color, 1, Self::hour_minute_to_angle(hour, minute), -60).anyhow()?;
        Self::draw_hand(manager.display(), &self.clock_face, clock_color, 1, Self::sexagesimal_to_angle(minute), -40).anyhow()?;

        let seconds_radians = Self::sexagesimal_to_angle(second);
        Self::draw_hand(manager.display(), &self.clock_face, clock_color, 1, seconds_radians, -11).anyhow()?;
        Self::draw_second_decoration(manager.display(), &self.clock_face, clock_color, bg_color, seconds_radians, -30).anyhow()?;

        // Draw a small circle over the hands in the center of the clock face.
        // This has to happen after the hands are drawn so they're covered up.
        Circle::with_center(self.clock_face.center(), 9)
            .into_styled(PrimitiveStyle::with_fill(clock_color))
            .draw(manager.display()).anyhow()?;

        self.hour = hour;
        self.minute = minute;
        self.second = second;

        Ok(())
    }

    pub fn update<'a, DM>(&mut self, manager: &'a mut DM)  -> anyhow::Result<()>
    where
        DM: DisplayManager,
    {
        self.do_update(manager, Local::now())

    }

    pub fn do_update<'a, DM>(&mut self, manager: &'a mut DM, now: chrono::DateTime<Local>)  -> anyhow::Result<()>
    where
        DM: DisplayManager,
    {
        let clock_color = manager.map_color(&self.clock_color);
        let bg_color = manager.map_color(&self.bg_color);

        

        let hour = now.hour();
        let minute = now.minute();
        let second = now.second();

        if hour != self.hour {
           
            self.hour = hour;
        }

        if minute != self.minute {
            Self::draw_hand(manager.display(), &self.clock_face, bg_color, 3, Self::hour_minute_to_angle(self.hour, self.minute), -60).anyhow()?;
            Self::draw_hand(manager.display(), &self.clock_face, bg_color, 3, Self::sexagesimal_to_angle(self.minute), -30).anyhow()?;
            self.minute = minute;
        }

        if second != self.second {
            let seconds_radians = Self::sexagesimal_to_angle(self.second);
            Self::draw_hand(manager.display(), &self.clock_face, bg_color, 3, seconds_radians, -11).anyhow()?;
            Self::draw_second_decoration(manager.display(), &self.clock_face, bg_color, bg_color, seconds_radians, -30).anyhow()?;

            self.second = second;
        }

        Self::draw_hand(manager.display(), &self.clock_face, clock_color, 1, Self::hour_minute_to_angle(hour, minute), -60).anyhow()?;
        Self::draw_hand(manager.display(), &self.clock_face, clock_color, 1, Self::sexagesimal_to_angle(minute), -40).anyhow()?;

        let seconds_radians = Self::sexagesimal_to_angle(second);
        Self::draw_hand(manager.display(), &self.clock_face, clock_color, 1, seconds_radians, -11).anyhow()?;
        Self::draw_second_decoration(manager.display(), &self.clock_face, clock_color, bg_color, seconds_radians, -30).anyhow()?;

        // Draw a small circle over the hands in the center of the clock face.
        // This has to happen after the hands are drawn so they're covered up.
        Circle::with_center(self.clock_face.center(), 9)
            .into_styled(PrimitiveStyle::with_fill(clock_color))
            .draw(manager.display()).anyhow()?;

        self.hour = hour;
        self.minute = minute;
        self.second = second;

        Ok(())
    }

    /// Converts a polar coordinate (angle/distance) into an (X, Y) coordinate centered around the
    /// center of the circle.
    ///
    /// The angle is relative to the 12 o'clock position and the radius is relative to the edge of the
    /// clock face.
    pub fn polar(circle: &Circle, angle: f32, radius_delta: i32) -> Point {
        let radius = circle.diameter as f32 / 2.0 + radius_delta as f32;

        circle.center()
            + Point::new(
                (angle.sin() * radius) as i32,
                -(angle.cos() * radius) as i32,
            )
    }

    /// Converts an hour into an angle in radians.
    pub fn hour_to_angle(hour: u32) -> f32 {
        // Convert from 24 to 12 hour time.
        let hour = hour % 12;

        (hour as f32 / 12.0) * 2.0 * PI
    }

    /// Converts an hour into an angle in radians.
    pub fn hour_minute_to_angle(hour: u32, minute: u32) -> f32 {
        // Convert from 24 to 12 hour time.
        let hour = hour % 12;

        ((hour as f32 + (minute as f32 / 60.0)) / 12.0)  * 2.0 * PI
    }

    /// Converts a sexagesimal (base 60) value into an angle in radians.
    pub fn sexagesimal_to_angle(value: u32) -> f32 {
        (value as f32 / 60.0) * 2.0 * PI
    }

    /// Creates a centered circle for the clock face.
    pub fn create_face(target: &impl DrawTarget) -> Circle {
        // The draw target bounding box can be used to determine the size of the display.
        let bounding_box = target.bounding_box();

        let diameter = bounding_box.size.width.min(bounding_box.size.height) - 2 * MARGIN;

        Circle::with_center(bounding_box.center(), diameter)
    }

    /// Draws a circle and 12 graduations as a simple clock face.
    pub fn draw_face<D,C>(target: &mut D, clock_face: &Circle, stroke_color: C) -> Result<(), D::Error>
    where
        C: PixelColor,
        D: DrawTarget<Color = C>,
    {
        // Draw the outer face.
        let style = PrimitiveStyle::with_stroke(stroke_color, 2);
        (*clock_face)
            .into_styled(style)
            .draw(target)?;

        // Draw 12 graduations.
        for angle in (0..12).map(Self::hour_to_angle) {
            // Start point on circumference.
            let start = Self::polar(clock_face, angle, 0);

            // End point offset by 10 pixels from the edge.
            let end = Self::polar(clock_face, angle, -10);

            Line::new(start, end)
                .into_styled(style)
                .draw(target)?;
        }

        Ok(())
    }

    /// Draws a clock hand.
    pub fn draw_hand<D,C>(
        target: &mut D,
        clock_face: &Circle,
        stroke_color: C,
        stroke_width: u32,
        angle: f32,
        length_delta: i32,
    ) -> Result<(), D::Error>
    where
        C: PixelColor,
        D: DrawTarget<Color = C>,
    {
        let end = Self::polar(clock_face, angle, length_delta);

        Line::new(clock_face.center(), end)
            .into_styled(PrimitiveStyle::with_stroke(stroke_color, stroke_width))
            .draw(target)
    }

    /// Draws a decorative circle on the second hand.
    pub fn draw_second_decoration<D,C>(
        target: &mut D,
        clock_face: &Circle,
        stroke_color: C,
        bg_color: C,
        angle: f32,
        length_delta: i32,
    ) -> Result<(), D::Error>
    where
        C: PixelColor,
        D: DrawTarget<Color = C>,
    {
        let decoration_position = Self::polar(clock_face, angle, length_delta);

        let decoration_style = PrimitiveStyleBuilder::new()
            .fill_color(bg_color)
            .stroke_color(stroke_color)
            .stroke_width(1)
            .build();

        // Draw a fancy circle near the end of the second hand.
        Circle::with_center(decoration_position, 11)
            .into_styled(decoration_style)
            .draw(target)
    }
}