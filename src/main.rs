mod app;
mod pet;
mod save;
mod species;
mod theme;
mod ui;

use std::time::{Duration, Instant};

use app::App;
use pet::Pet;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TICK: Duration = Duration::from_millis(100);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

fn main() -> Result<()> {
    let pet = save::load()?.unwrap_or_else(Pet::new);
    let mut app = App::new(pet, species::builtin().remove(0), speed());
    let result = run(&mut ratatui::init(), &mut app);
    ratatui::restore();
    save::store(&app.pet)?;
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();
    let mut last_save = last_tick;
    while !app.quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
            save::store(&app.pet)?;
            last_save = Instant::now();
        }
        let now = Instant::now();
        app.tick(now - last_tick);
        last_tick = now;
        if now - last_save >= AUTOSAVE_INTERVAL {
            save::store(&app.pet)?;
            last_save = now;
        }
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
