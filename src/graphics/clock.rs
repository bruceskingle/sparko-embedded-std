//! A simple analog clock example using embedded-graphics.
//! Based on https://github.com/embedded-graphics/examples/blob/main/eg-0.8/examples/demo-analog-clock.rs

use core::f32::consts::PI;
use chrono::{Local, Timelike};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder},
};

use crate::graphics::{DisplayManager, DrawTargetResultExt};


use embedded_graphics::prelude::DrawTarget;

enum DrawMode {
    Draw,
    Erase,
}

pub struct ClockRenderer{
    clock_face: Circle,
    radius: f32,
    tick_start: f32,
    sec_hand_len: f32,
    sec_dec_len: f32,
    sec_dec_dia: u32,
    min_hand_len: f32,
    hour_hand_len: f32,
    centre_dia: u32,
    clock_color: super::Color,
    bg_color: super::Color,
    hour: u32,
    minute: u32,
    second: u32,
}

impl ClockRenderer
{
    pub fn new<DM>(manager: &mut DM) -> anyhow::Result<Self> 
    where
        DM: DisplayManager,
    {
        let target: &mut DM::Display = manager.display();

        // The draw target bounding box can be used to determine the size of the display.
        let bounding_box = target.bounding_box();

        let margin = 1;
        let diameter = bounding_box.size.width.min(bounding_box.size.height) - 2 * margin;
        let radius = (diameter / 2) as f32;

        let clock_face = Circle::with_center(bounding_box.center(), diameter);
        let tick_start = radius * 0.9;
        let sec_hand_len = radius * 0.8;
        let sec_dec_len = radius * 0.6;
        let sec_dec_dia = (radius * 0.15) as u32;
        let min_hand_len = radius * 0.70;
        let hour_hand_len = radius * 0.60;
        let centre_dia = (radius * 0.2) as u32;


        // let clock_color = manager.map_color(&super::Color::Green);
        // let bg_color = manager.map_color(&super::Color::Black);

        // let clock_face = Self::create_face(target);
        let mut clock_renderer = ClockRenderer {
            clock_face,
            radius,
            tick_start,
            sec_hand_len,
            sec_dec_len,
            sec_dec_dia,
            min_hand_len,
            hour_hand_len,
            centre_dia,
            clock_color: super::Color::Green,
            bg_color: super::Color::Black,
            hour: 0,
            minute: 0,
            second: 0,
        };

        clock_renderer.draw(manager)?;

        Ok(clock_renderer)
    }

    fn context<'a, DM: DisplayManager>(&'a mut self, manager: &'a mut DM) -> Context<'a, DM> {
        let clock_color = manager.map_color(&self.clock_color);
        let bg_color = manager.map_color(&self.bg_color);
        let target = manager.display();
        Context {
            clock_renderer: self,
            target,
            clock_color,
            bg_color,
        }
    }

    pub fn draw<'a, DM: DisplayManager>(&'a mut self, manager: &mut DM)  -> anyhow::Result<()> {
        self.context(manager).draw()
    }

    pub fn update<'a, DM: DisplayManager>(&'a mut self, manager: &mut DM)  -> anyhow::Result<()> {
        self.context(manager).update()
    }
}



struct Context<'a, DM: DisplayManager> {
    clock_renderer: &'a mut ClockRenderer,
    target: &'a mut <DM as DisplayManager>::Display,
    clock_color: <<DM>::Display as DrawTarget>::Color,
    bg_color: <<DM>::Display as DrawTarget>::Color,
}

impl<'a, DM: DisplayManager> Context<'a, DM> {

    pub fn draw(&mut self)  -> anyhow::Result<()>
    where
        DM: DisplayManager,
    {
        self.clear()?;

        let now = Local::now();

        let hour = now.hour();
        let minute = now.minute();
        let second = now.second();


        self.draw_face()?;
        // self.draw_hour_hand(&DrawMode::Draw, Self::hour_minute_to_angle(hour, minute))?;
        // self.draw_min_hand(&DrawMode::Draw, Self::sexagesimal_to_angle(minute))?;
        // self.draw_sec_hand(&DrawMode::Draw, Self::sexagesimal_to_angle(second))?;

        // Self::draw_hand(manager.display(), &self.clock_face, self.clock_color, 1, Self::hour_minute_to_angle(hour, minute), -60).anyhow()?;
        // Self::draw_hand(manager.display(), &self.clock_face, self.clock_color, 1, Self::sexagesimal_to_angle(minute), -40).anyhow()?;

        // let seconds_radians = Self::sexagesimal_to_angle(second);
        // Self::draw_hand(manager.display(), &self.clock_face, self.clock_color, 1, seconds_radians, -11).anyhow()?;
        // Self::draw_second_decoration(manager.display(), &self.clock_face, self.clock_color, self.bg_color, seconds_radians, -30).anyhow()?;

        // Draw a small circle over the hands in the center of the clock face.
        // This has to happen after the hands are drawn so they're covered up.
        Circle::with_center(self.clock_renderer.clock_face.center(), self.clock_renderer.centre_dia)
            .into_styled(PrimitiveStyle::with_fill(self.clock_color))
            .draw(self.target).anyhow()?;

        self.clock_renderer.hour = hour;
        self.clock_renderer.minute = minute;
        self.clock_renderer.second = second;

        Ok(())
    }

    pub fn update(&mut self)  -> anyhow::Result<()>
    {
        let now = Local::now();
        let hour = now.hour();
        let minute = now.minute();
        let second = now.second();


        if minute != self.clock_renderer.minute {
            // Self::draw_hand(manager.display(), &self.clock_face, self.bg_color, 3, Self::hour_minute_to_angle(self.hour, self.minute), -60).anyhow()?;
            // Self::draw_hand(manager.display(), &self.clock_face, self.bg_color, 3, Self::sexagesimal_to_angle(self.minute), -30).anyhow()?;
            self.draw_hour_hand(&DrawMode::Erase, Self::hour_minute_to_angle(self.clock_renderer.hour, self.clock_renderer.minute))?;
            self.draw_min_hand(&DrawMode::Erase, Self::sexagesimal_to_angle(self.clock_renderer.minute))?;
        }

        if second != self.clock_renderer.second {
            // let seconds_radians = Self::sexagesimal_to_angle(self.second);
            // Self::draw_hand(manager.display(), &self.clock_face, self.bg_color, 3, seconds_radians, -11).anyhow()?;
            // Self::draw_second_decoration(manager.display(), &self.clock_face, self.bg_color, self.bg_color, seconds_radians, -30).anyhow()?;

            self.draw_sec_hand(&DrawMode::Erase, Self::sexagesimal_to_angle(self.clock_renderer.second))?;
        }


        self.draw_hour_hand(&DrawMode::Draw, Self::hour_minute_to_angle(hour, minute))?;
        self.draw_min_hand(&DrawMode::Draw, Self::sexagesimal_to_angle(minute))?;
        self.draw_sec_hand(&DrawMode::Draw, Self::sexagesimal_to_angle(second))?;

        // Self::draw_hand(manager.display(), &self.clock_face, self.clock_color, 1, Self::hour_minute_to_angle(hour, minute), -60).anyhow()?;
        // Self::draw_hand(manager.display(), &self.clock_face, self.clock_color, 1, Self::sexagesimal_to_angle(minute), -40).anyhow()?;

        // let seconds_radians = Self::sexagesimal_to_angle(second);
        // Self::draw_hand(manager.display(), &self.clock_face, self.clock_color, 1, seconds_radians, -11).anyhow()?;
        // Self::draw_second_decoration(manager.display(), &self.clock_face, self.clock_color, self.bg_color, seconds_radians, -30).anyhow()?;

        // Draw a small circle over the hands in the center of the clock face.
        // This has to happen after the hands are drawn so they're covered up.
        Circle::with_center(self.clock_renderer.clock_face.center(), self.clock_renderer.centre_dia)
            .into_styled(PrimitiveStyle::with_fill(self.clock_color))
            .draw(self.target).anyhow()?;

        self.clock_renderer.hour = hour;
        self.clock_renderer.minute = minute;
        self.clock_renderer.second = second;

        Ok(())
    }

    // fn fill_color(&mut self, manager: &mut DM, color: <<DM>::Display as DrawTarget>::Color) -> anyhow::Result<()> {
    //     manager.display().bounding_box()
    //         .into_styled(PrimitiveStyle::with_fill(color))
    //         .draw(manager.display()).anyhow()
    // }

    fn clear(&mut self)-> anyhow::Result<()> {
        self.target.bounding_box()
            .into_styled(PrimitiveStyle::with_fill(self.bg_color))
            .draw(self.target).anyhow()

        // let manager: &DM = self.manager;
        // self.fill_color(self.manager, self.bg_color)
    }

    // /// Converts a polar coordinate (angle/distance) into an (X, Y) coordinate centered around the
    // /// center of the circle.
    // ///
    // /// The angle is relative to the 12 o'clock position and the radius is relative to the edge of the
    // /// clock face.
    // pub fn polar(circle: &Circle, angle: f32, radius: f32) -> Point {
    //     // let radius = circle.diameter as f32 / 2.0 + radius_delta as f32;

    //     circle.center()
    //         + Point::new(
    //             (angle.sin() * radius) as i32,
    //             -(angle.cos() * radius) as i32,
    //         )
    // }

    /// Converts a polar coordinate (angle/distance) into an (X, Y) coordinate centered around the
    /// center of the circle.
    ///
    /// The angle is relative to the 12 o'clock position and the radius is relative to the edge of the
    /// clock face.
    fn polar(&self, angle: f32, radius: f32) -> Point {
        self.clock_renderer.clock_face.center()
            + Point::new(
                (angle.sin() * radius) as i32,
                -(angle.cos() * radius) as i32,
            )
    }

    /// Converts an hour into an angle in radians.
    fn hour_to_angle(hour: u32) -> f32 {
        // Convert from 24 to 12 hour time.
        let hour = hour % 12;

        (hour as f32 / 12.0) * 2.0 * PI
    }

    /// Converts an hour into an angle in radians.
    fn hour_minute_to_angle(hour: u32, minute: u32) -> f32 {
        // Convert from 24 to 12 hour time.
        let hour = hour % 12;

        ((hour as f32 + (minute as f32 / 60.0)) / 12.0)  * 2.0 * PI
    }

    /// Converts a sexagesimal (base 60) value into an angle in radians.
    fn sexagesimal_to_angle(value: u32) -> f32 {
        (value as f32 / 60.0) * 2.0 * PI
    }

    // /// Creates a centered circle for the clock face.
    // fn create_face(target: &impl DrawTarget) -> Circle {
    //     // The draw target bounding box can be used to determine the size of the display.
    //     let bounding_box = target.bounding_box();

    //     let diameter = bounding_box.size.width.min(bounding_box.size.height) - 2 * MARGIN;

    //     Circle::with_center(bounding_box.center(), diameter)
    // }

    /// Draws a circle and 12 graduations as a simple clock face.
    fn draw_face(&mut self) -> anyhow::Result<()>

    {
        // Draw the outer face.
        let style = PrimitiveStyle::with_stroke(self.clock_color, 2);
        (self.clock_renderer.clock_face)
            .into_styled(style)
            .draw(self.target).anyhow()?;

        // Draw 12 graduations.
        for angle in (0..12).map(Self::hour_to_angle) {
            // Start point on circumference.
            let start = self.polar(angle, self.clock_renderer.radius);

            // End point offset by 10 pixels from the edge.
            let end = self.polar(angle, self.clock_renderer.tick_start);

            Line::new(start, end)
                .into_styled(style)
                .draw(self.target).anyhow()?;
        }

        Ok(())
    }

    // /// Draws a circle and 12 graduations as a simple clock face.
    // pub fn draw_face<D,C>(target: &mut D, clock_face: &Circle, stroke_color: C) -> Result<(), D::Error>
    // where
    //     C: PixelColor,
    //     D: DrawTarget<Color = C>,
    // {
    //     // Draw the outer face.
    //     let style = PrimitiveStyle::with_stroke(stroke_color, 2);
    //     (*clock_face)
    //         .into_styled(style)
    //         .draw(target)?;

    //     // Draw 12 graduations.
    //     for angle in (0..12).map(Self::hour_to_angle) {
    //         // Start point on circumference.
    //         let start = Self::polar(clock_face, angle, 0);

    //         // End point offset by 10 pixels from the edge.
    //         let end = Self::polar(clock_face, angle, -10);

    //         Line::new(start, end)
    //             .into_styled(style)
    //             .draw(target)?;
    //     }

    //     Ok(())
    // }

    /// Draws a clock hand.
    fn draw_hand(
        &mut self,
        stroke_color: <<DM>::Display as DrawTarget>::Color,
        stroke_width: u32,
        angle: f32,
        length: f32,
    ) -> anyhow::Result<()>
    {
        let end = self.polar(angle, length);

        Line::new(self.clock_renderer.clock_face.center(), end)
            .into_styled(PrimitiveStyle::with_stroke(stroke_color, stroke_width))
            .draw(self.target).anyhow()
    }

    fn fg_color(&self, draw_mode: &DrawMode) -> <<DM>::Display as DrawTarget>::Color {
        match draw_mode {
            DrawMode::Draw => self.clock_color,
            DrawMode::Erase => self.bg_color,
        }
    }

    fn stroke_width(&self, draw_mode: &DrawMode) -> u32 {
        match draw_mode {
            DrawMode::Draw => 1,
            DrawMode::Erase => 3,
        }
    }

    fn draw_hour_hand(
        &mut self,
        draw_mode: &DrawMode,
        angle: f32,
    ) -> anyhow::Result<()>
    {
        self.draw_hand(self.fg_color(draw_mode), 3, angle, self.clock_renderer.hour_hand_len)
    }

    fn draw_min_hand(
        &mut self,
        draw_mode: &DrawMode,
        angle: f32,
    ) -> anyhow::Result<()>
    {
        self.draw_hand(self.fg_color(draw_mode), 2, angle, self.clock_renderer.min_hand_len)
    }

    fn draw_sec_hand(
        &mut self,
        draw_mode: &DrawMode,
        angle: f32,
    ) -> anyhow::Result<()>
    {
        self.draw_hand(self.fg_color(draw_mode), 1, angle, self.clock_renderer.sec_hand_len)?;
        self.draw_second_decoration(draw_mode, angle, self.clock_renderer.sec_dec_len)
    }

    /// Draws a decorative circle on the second hand.
    fn draw_second_decoration(
        &mut self,
        draw_mode: &DrawMode,
        angle: f32,
        length: f32,
    ) -> anyhow::Result<()>
    {
        let decoration_position = self.polar(angle, length);

        let decoration_style = PrimitiveStyleBuilder::new()
            .fill_color(self.bg_color)
            .stroke_color(self.fg_color(draw_mode))
            .stroke_width(1)
            .build();

        // Draw a fancy circle near the end of the second hand.
        Circle::with_center(decoration_position, self.clock_renderer.sec_dec_dia)
            .into_styled(decoration_style)
            .draw(self.target).anyhow()

    }
}