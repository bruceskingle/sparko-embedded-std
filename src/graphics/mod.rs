use embedded_graphics::prelude::DrawTarget;
use anyhow::Result;

use crate::{InitStatus, Status};

mod clock;
pub use clock::*;

pub enum Color {
    Black,
    Red,
    Green,
    Blue,
    Yellow,
    Magenta,
    Cyan,
    White,
}


pub trait DrawTargetResultExt<T> {
    fn anyhow(self) -> Result<T>;
}

impl<T, E> DrawTargetResultExt<T> for Result<T, E> {
    fn anyhow(self) -> Result<T> {
        self.map_err(|_| anyhow::anyhow!("DrawTarget operation failed"))
    }
}

pub trait DisplayManager {
    type Display: DrawTarget;

    fn display(&mut self) -> &mut Self::Display;
    
    fn set_status(&mut self, status: &Status) -> anyhow::Result<()> {
        match status  {
            Status::Initializing(init_status) => {
                match init_status {
                    InitStatus::Starting => self.fill_color(Color::Yellow),
                    InitStatus::AwaitingClientIpAddress => self.fill_color(Color::Magenta),
                    InitStatus::AwaitingTimeSync => self.fill_color(Color::Cyan),
                    InitStatus::StartingFeatures => self.fill_color(Color::Black),
                }
            },
            Status::Running => Ok(()),
            Status::Setup => self.fill_color(Color::Blue),
            Status::Error => self.fill_color(Color::Red),
        }
    }

    fn fill_color(&mut self, color: Color) -> anyhow::Result<()>;

    fn map_color(&self, color: &Color) -> <Self::Display as DrawTarget>::Color;
}