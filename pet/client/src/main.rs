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
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Position, Rect};

use crate::app::{App, Click};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// 10 fps
const TICK: Duration = Duration::from_millis(100);
/// a paste comes as a burst of keys
const BURST_GAP: Duration = Duration::from_millis(5);

/// a captured mouse stops right clicks from pasting
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

    let mut terminal = ratatui::init();
    _ = execute!(std::io::stdout(), EnableMouseCapture);
    let result = run(&mut terminal, &mut app);
    _ = execute!(std::io::stdout(), DisableMouseCapture);
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
        let mut clickable = Vec::new();
        terminal.draw(|frame| clickable = ui::draw(frame, app))?;
        if event::poll(TICK)? {
            match read_burst(app, &clickable)?.as_slice() {
                [] => {}
                [key] => app.on_key(*key),
                keys => app.on_paste(&keys.iter().filter_map(|key| key.code.as_char()).collect::<String>()),
            }
            sign_up(app);
        }
        let now = Instant::now();
        app.tick(now - last_tick);
        last_tick = now;
    }
    Ok(())
}

/// keys are gathered to tell typing from a paste, a click ends it since the screen may change
fn read_burst(app: &mut App, clickable: &[(Rect, Click)]) -> Result<Vec<KeyEvent>> {
    let mut keys = Vec::new();
    loop {
        let gap = match event::read()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    keys.push(key);
                }
                BURST_GAP
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                ..
            }) => {
                let at = Position::new(column, row);
                if let Some(&(_, click)) = clickable.iter().find(|(area, _)| area.contains(at)) {
                    app.on_click(click);
                }
                return Ok(keys);
            }
            _ => Duration::ZERO,
        };
        if !event::poll(gap)? {
            return Ok(keys);
        }
    }
}
