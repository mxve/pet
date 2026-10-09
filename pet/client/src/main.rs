/*!
pet client:
  flags and startup
  sign up
  main loop
*/

mod account;
mod app;
mod bar;
mod floaters;
mod online;
mod species;
mod theme;
mod ui;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::app::App;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// 10 fps
const TICK: Duration = Duration::from_millis(100);

fn main() -> Result<()> {
    if let Some(address) = flag("--ping") {
        return online::ping(&address);
    }

    #[cfg(debug_assertions)]
    let dev = std::env::args().any(|argument| argument == "--dev");
    #[cfg(not(debug_assertions))]
    let dev = false;
    let account = if dev { None } else { account::load()? };
    let remote = match &account {
        Some(account) => online::follow(account)?,
        None => None,
    };

    let mut app = App::new(species::builtin(), remote, now());
    #[cfg(debug_assertions)]
    if dev {
        app.start_dev();
    }

    let result = run(&mut ratatui::init(), &mut app);
    ratatui::restore();
    result
}

fn now() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
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
    #[cfg(debug_assertions)]
    if app.dev {
        return;
    }
    let Some(pet) = app.pet() else {
        return;
    };

    let followed = online::register(&pet.name, &pet.species).and_then(|account| match account {
        Some(account) => {
            account::store(&account)?;
            online::follow(&account)
        }
        None => Ok(None),
    });
    match followed {
        Ok(remote) => app.remote = remote,
        Err(problem) => app.say(&format!("could not sign up: {problem}")),
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();
    while !app.quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
            sign_up(app);
        }
        let now = Instant::now();
        app.tick(now - last_tick);
        last_tick = now;
    }
    Ok(())
}
