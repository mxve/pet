mod account;
mod app;
mod bar;
mod online;
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
    if let Some(address) = flag("--ping") {
        return online::ping(&address);
    }
    #[cfg(debug_assertions)]
    let dev = std::env::args().any(|argument| argument == "--dev");
    #[cfg(not(debug_assertions))]
    let dev = false;
    let species = species::builtin();
    let world = if dev { None } else { save::load()? };
    let mut choice = 0;
    if let Some(world) = &world {
        let pet = world.pet();
        let known = species.iter().position(|species| species.name == pet.species);
        let Some(known) = known else {
            let path = save::path()?;
            return Err(format!("{}: unknown species \"{}\"", path.display(), pet.species).into());
        };
        choice = known;
    }
    let mut app = App::new(world, species, choice, save::now());
    app.catch_up();
    #[cfg(debug_assertions)]
    if dev {
        app.start_dev();
    }
    let result = run(&mut ratatui::init(), &mut app);
    ratatui::restore();
    store(&mut app)?;
    result
}

fn flag(name: &str) -> Option<String> {
    let mut arguments = std::env::args().skip_while(|argument| argument != name);
    arguments.next()?;
    arguments.next()
}

fn sign_up(app: &mut App) {
    if !app.take_adopted() {
        return;
    }
    let Some(pet) = app.pet() else {
        return;
    };
    let (name, species) = (pet.name.clone(), pet.species.clone());
    let problem = match online::register(&name, &species) {
        Ok(Some(account)) => account::store(&account).err(),
        Ok(None) => None,
        Err(error) => Some(error),
    };
    if let Some(problem) = problem {
        app.say(&format!("could not sign up: {problem}"));
    }
}

fn store(app: &mut App) -> Result<()> {
    #[cfg(debug_assertions)]
    if app.dev {
        return Ok(());
    }
    match &mut app.world {
        Some(world) => save::store(world),
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
            sign_up(app);
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
