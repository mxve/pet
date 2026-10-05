use std::time::Duration;

use ratatui::text::{Line, Span};

use crate::random::roll;
use crate::theme::{self, LAVENDER, MUTED, PINK, TEXT};

const SPARKLES: [&str; 5] = ["₊", ".", "⋆", "+", "✧"];
const EMPTY: &str = "┄";
const TWINKLE: Duration = Duration::from_millis(350);
const FLASH_CHANCE: u64 = 6;

pub fn xp_bar(ratio: f32, width: u16, clock: Duration) -> Line<'static> {
    let width = usize::from(width);
    let filled = (ratio.clamp(0.0, 1.0) * width as f32).round() as usize;
    (0..width)
        .map(|cell| {
            if cell < filled {
                sparkle(cell, width, clock)
            } else {
                Span::styled(EMPTY, MUTED)
            }
        })
        .collect()
}

fn sparkle(cell: usize, width: usize, clock: Duration) -> Span<'static> {
    let period = TWINKLE.as_millis();
    let offset = u128::from(roll(("offset", cell))) % period;
    let dice = roll((cell, (clock.as_millis() + offset) / period));
    let glyph = SPARKLES[dice as usize % SPARKLES.len()];
    let color = if (dice / SPARKLES.len() as u64).is_multiple_of(FLASH_CHANCE) {
        TEXT
    } else {
        theme::mix(PINK, LAVENDER, cell as f32 / width as f32)
    };
    Span::styled(glyph, color)
}
