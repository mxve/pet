use std::time::Duration;

use ratatui::style::Color;
use ratatui::text::{Line, Span};

use crate::theme::{self, LAVENDER, MUTED, PINK, TEXT};
use pet_core::random::roll;

const SPARKLES: &[&str] = &["₊", ".", "⋆", "+", "✧"];
const SMALL_SPARKLES: &[&str] = &["·", "·", "·", "·", "·", "₊", "."];
const EMPTY: &str = "┄";
const FADE: f32 = 0.5;
const TWINKLE: Duration = Duration::from_millis(350);
const FLASH_CHANCE: u64 = 6;

pub struct Bar {
    name: &'static str,
    ratio: f32,
    colors: (Color, Color),
    glyphs: &'static [&'static str],
}

impl Bar {
    pub fn new(name: &'static str, ratio: f32) -> Bar {
        Bar {
            name,
            ratio,
            colors: (PINK, LAVENDER),
            glyphs: SPARKLES,
        }
    }

    pub fn small(self) -> Bar {
        Bar {
            glyphs: SMALL_SPARKLES,
            ..self
        }
    }

    pub fn fading_to(self, color: Color) -> Bar {
        Bar {
            colors: (theme::mix(color, MUTED, FADE), color),
            ..self
        }
    }

    pub fn line(&self, width: u16, clock: Duration) -> Line<'static> {
        let width = usize::from(width);
        let filled = filled_cells(self.ratio, width);
        (0..width)
            .map(|cell| {
                if cell < filled {
                    self.sparkle(cell, width, clock)
                } else {
                    Span::styled(EMPTY, MUTED)
                }
            })
            .collect()
    }

    fn sparkle(&self, cell: usize, width: usize, clock: Duration) -> Span<'static> {
        let period = TWINKLE.as_millis();
        let offset = u128::from(roll((self.name, "offset", cell))) % period;
        let dice = roll((self.name, cell, (clock.as_millis() + offset) / period));
        let glyph = self.glyphs[dice as usize % self.glyphs.len()];
        let color = if (dice / self.glyphs.len() as u64).is_multiple_of(FLASH_CHANCE) {
            TEXT
        } else {
            let (from, to) = self.colors;
            theme::mix(from, to, cell as f32 / width as f32)
        };
        Span::styled(glyph, color)
    }
}

fn filled_cells(ratio: f32, width: usize) -> usize {
    if ratio <= 0.0 {
        0
    } else {
        ((ratio * width as f32).round() as usize).clamp(1, width)
    }
}
