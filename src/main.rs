mod app;
mod bar;
mod pet;
mod random;
mod save;
mod species;
mod theme;
mod ui;

use std::time::{Duration, Instant};

use app::App;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TICK: Duration = Duration::from_millis(100);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

fn main() -> Result<()> {
    let species = species::builtin();
    let mut pet = save::load()?;
    let mut choice = 0;
    if let Some(pet) = &mut pet {
        let known = species
            .iter()
            .position(|species| species.name == pet.species);
        let Some(known) = known else {
            let path = save::path()?;
            return Err(format!("{}: unknown species \"{}\"", path.display(), pet.species).into());
        };
        choice = known;
        let away = save::now().saturating_sub(pet.last_seen);
        pet.advance(Duration::from_secs(away));
    }
    let mut app = App::new(pet, species, choice, speed());
    let result = run(&mut ratatui::init(), &mut app);
    ratatui::restore();
    store(&mut app)?;
    result
}

fn store(app: &mut App) -> Result<()> {
    match &mut app.pet {
        Some(pet) => save::store(pet),
        None => Ok(()),
    }
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
            store(app)?;
            last_save = Instant::now();
        }
        let now = Instant::now();
        app.tick(now - last_tick);
        last_tick = now;
        if now - last_save >= AUTOSAVE_INTERVAL {
            store(app)?;
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
