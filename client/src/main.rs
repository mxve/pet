mod app;
mod bar;
mod save;
mod species;
mod theme;
mod ui;

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use app::App;
use pet_core::protocol::{self, PORT, Packet};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TICK: Duration = Duration::from_millis(100);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);
const PING_TIMEOUT: Duration = Duration::from_secs(2);

fn main() -> Result<()> {
    if let Some(address) = flag("--ping") {
        return ping(&address);
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

fn ping(address: &str) -> Result<()> {
    let address = if address.contains(':') {
        address.to_string()
    } else {
        format!("{address}:{PORT}")
    };
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(PING_TIMEOUT))?;
    socket.send_to(&protocol::get_info(), &address)?;
    let mut buffer = [0; protocol::MAX_PACKET + 1];
    let size = socket
        .recv(&mut buffer)
        .map_err(|error| format!("{address}: no answer ({error})"))?;
    match protocol::decode(&buffer[..size]) {
        Some(Packet::Info(info)) => println!(
            "{address}: {}, {} players (protocol {})",
            info.name,
            info.players,
            protocol::VERSION
        ),
        _ => return Err(format!("{address}: answered with something that is not info").into()),
    }
    Ok(())
}

fn flag(name: &str) -> Option<String> {
    let mut arguments = std::env::args().skip_while(|argument| argument != name);
    arguments.next()?;
    arguments.next()
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
