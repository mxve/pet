mod app;
mod pet;
mod species;
mod theme;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use app::App;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TICK: Duration = Duration::from_millis(100);

fn main() -> io::Result<()> {
    let result = run(&mut ratatui::init());
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new(species::builtin().remove(0), speed());
    let mut last_tick = Instant::now();
    while !app.quit {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
        let now = Instant::now();
        app.tick(now - last_tick);
        last_tick = now;
    }
    Ok(())
}

fn speed() -> f32 {
    std::env::var("PET_SPEED")
        .ok()
        .and_then(|speed| speed.parse().ok())
        .filter(|speed| *speed > 0.0)
        .unwrap_or(1.0)
}
