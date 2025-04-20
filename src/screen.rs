use core::fmt::Write as _;

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embedded_graphics::{
    mono_font::{ascii, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::{Alignment, Text},
};

use crate::platform::Screen;

pub enum ScreenState {
    OnUntil { hour: i8, min: i8 },
    OffUntil { hour: i8, min: i8 },
}

#[embassy_executor::task]
pub async fn drive_screen(signal: &'static Signal<NoopRawMutex, ScreenState>, mut display: Screen) {
    loop {
        let next_state = signal.wait().await;
        display.clear(Rgb565::BLACK).unwrap();
        match next_state {
            ScreenState::OnUntil { hour, min } => {
                let mut buf = heapless::String::<20>::new();
                if let Ok(()) = write!(&mut buf, "on until {hour}:{min:02}") {
                    let style = MonoTextStyle::new(&ascii::FONT_10X20, Rgb565::WHITE);
                    let text = Text::with_alignment(
                        buf.as_str(),
                        display.bounding_box().center(),
                        style,
                        Alignment::Center,
                    );
                    text.draw(&mut display).unwrap();
                }
            }
            ScreenState::OffUntil { hour, min } => {
                let mut buf = heapless::String::<20>::new();
                if let Ok(()) = write!(&mut buf, "off until {hour}:{min:02}") {
                    let style = MonoTextStyle::new(&ascii::FONT_10X20, Rgb565::WHITE);
                    let text = Text::with_alignment(
                        buf.as_str(),
                        display.bounding_box().center(),
                        style,
                        Alignment::Center,
                    );
                    text.draw(&mut display).unwrap();
                }
            }
        };
    }
}
