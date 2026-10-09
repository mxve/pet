/*!
floaters:
  numbers that float up and fade
*/

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::Line;

use crate::theme::{self, MUTED};

const LIFE: Duration = Duration::from_millis(1500);
const SPREAD: [u16; 3] = [0, 3, 1];

struct Floater {
    text: String,
    color: Color,
    shift: u16,
    age: Duration,
}

#[derive(Default)]
pub struct Floaters {
    live: Vec<Floater>,
    spawned: usize,
}

impl Floaters {
    pub fn spawn(&mut self, text: String, color: Color) {
        let shift = SPREAD[self.spawned % SPREAD.len()];
        self.spawned += 1;
        self.live.push(Floater {
            text,
            color,
            shift,
            age: Duration::ZERO,
        });
    }

    pub fn tick(&mut self, elapsed: Duration) {
        for floater in &mut self.live {
            floater.age += elapsed;
        }
        self.live.retain(|floater| floater.age < LIFE);
    }

    pub fn draw(&self, frame: &mut Frame, beside: Rect) {
        let screen = frame.area();
        let start = beside.y + beside.height / 2;
        for floater in &self.live {
            let progress = floater.age.as_secs_f32() / LIFE.as_secs_f32();
            let risen = (progress * f32::from(start - beside.y)).round() as u16;
            let line = Line::styled(floater.text.as_str(), theme::mix(floater.color, MUTED, progress * progress));
            let x = beside.right() + 1 + floater.shift;
            let area = Rect::new(x, start - risen, line.width() as u16, 1).intersection(screen);
            frame.render_widget(line, area);
        }
    }
}
