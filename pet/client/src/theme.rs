/*!
theme:
  all the colors
  mixing them
*/

use ratatui::style::Color;

pub const PINK: Color = Color::Rgb(0xf5, 0xc2, 0xe7);
pub const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
pub const SKY: Color = Color::Rgb(0x89, 0xdc, 0xeb);
pub const MINT: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
pub const ROSE: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
pub const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
pub const LAVENDER: Color = Color::Rgb(0xb4, 0xbe, 0xfe);
pub const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
pub const MUTED: Color = Color::Rgb(0x6c, 0x70, 0x86);
pub const BASE: Color = Color::Rgb(0x1e, 0x1e, 0x1e);
pub const SURFACE: Color = Color::Rgb(0x25, 0x25, 0x26);
pub const BUTTON: Color = Color::Rgb(0x30, 0x30, 0x33);

/// only rgb mixes, anything else stays as is
pub fn mix(from: Color, to: Color, amount: f32) -> Color {
    let (Color::Rgb(red, green, blue), Color::Rgb(to_red, to_green, to_blue)) = (from, to) else {
        return from;
    };
    let channel = |from: u8, to: u8| {
        let (from, to) = (f32::from(from), f32::from(to));
        (from + (to - from) * amount).round() as u8
    };
    Color::Rgb(channel(red, to_red), channel(green, to_green), channel(blue, to_blue))
}
