use core::fmt::Write as _;

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii},
    pixelcolor::Rgb565,
    prelude::*,
    text::{Alignment, Text},
};
use jiff::civil::Time;

use crate::platform::Screen;

macro_rules! write_to_screen {
    ($display:expr, $dst:expr, $($arg:tt)*) => {
        {
            let mut buf = heapless::String::<20>::new();
            let display = &mut $display;
            if write!(&mut buf, $dst, $($arg)*).is_err() {
                buf.clear();
                write!(&mut buf, "[too long]").unwrap();
            }
            let style = MonoTextStyle::new(&ascii::FONT_10X20, Rgb565::WHITE);
            let text = Text::with_alignment(
                buf.as_str(),
                display.bounding_box().center(),
                style,
                Alignment::Center,
            );
            text.draw(display)
        }
    }
}

pub enum ScreenState {
    OnUntil(Time),
    OffUntil(Time),
}

#[embassy_executor::task]
pub async fn drive_screen(signal: &'static Signal<NoopRawMutex, ScreenState>, mut display: Screen) {
    loop {
        let next_state = signal.wait().await;
        display.clear(Rgb565::BLACK).unwrap();
        match next_state {
            ScreenState::OnUntil(time) => {
                write_to_screen!(display, "on until {}", time.strftime("%-I:%M%P")).unwrap();
            }
            ScreenState::OffUntil(time) => {
                write_to_screen!(display, "off until {}", time.strftime("%-I:%M%P")).unwrap();
            }
        };
    }
}
