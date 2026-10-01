mod pet;

use std::io;
use std::time::{Duration, Instant};

use pet::{Mood, Pet, Stats};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::line::THICK_HORIZONTAL;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, LineGauge};
use ratatui::{DefaultTerminal, Frame};

const TICK: Duration = Duration::from_millis(100);
const CARD_WIDTH: u16 = 49;
const CARD_HEIGHT: u16 = 13;
const CARD_PADDING: u16 = 3;

const PINK: Color = Color::Rgb(0xf5, 0xc2, 0xe7);
const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
const SKY: Color = Color::Rgb(0x89, 0xdc, 0xeb);
const MINT: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
const ROSE: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
const LAVENDER: Color = Color::Rgb(0xb4, 0xbe, 0xfe);
const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
const MUTED: Color = Color::Rgb(0x6c, 0x70, 0x86);

const NAME: &str = "Mochi";
const MESSAGE_TIME: Duration = Duration::from_millis(1500);
const HELP: &str = " f feed | p pet | y play | q quit ";

const FRAME_TIME: Duration = Duration::from_millis(400);
const SEQUENCE: [usize; 6] = [0, 0, 0, 0, 0, 1];
const FRAMES: [&str; 2] = [
    r" /\_/\
( o.o )
 > ^ <",
    r" /\_/\
( -.- )
 > ^ <",
];

fn main() -> io::Result<()> {
    let result = run(&mut ratatui::init());
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let speed = speed();
    let started = Instant::now();
    let mut last_tick = started;
    let mut pet = Pet::new();
    let mut acting: Option<(&str, Duration)> = None;
    loop {
        let clock = started.elapsed();
        terminal.draw(|frame| draw(frame, &pet, clock, acting.map(|(message, _)| message)))?;
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if is_quit(key) {
                return Ok(());
            }
            if let Some((effect, message)) = action(key.code) {
                pet.apply(effect);
                acting = Some((message, MESSAGE_TIME));
            }
        }
        let now = Instant::now();
        let elapsed = now - last_tick;
        pet.tick(elapsed.mul_f32(speed));
        acting = acting.and_then(|(message, left)| Some((message, left.checked_sub(elapsed)?)));
        last_tick = now;
    }
}

fn speed() -> f32 {
    std::env::var("PET_SPEED")
        .ok()
        .and_then(|speed| speed.parse().ok())
        .filter(|speed| *speed > 0.0)
        .unwrap_or(1.0)
}

fn is_quit(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

fn action(code: KeyCode) -> Option<(Stats, &'static str)> {
    match code {
        KeyCode::Char('f') => Some((
            Stats {
                food: 30.0,
                joy: 2.0,
                energy: 0.0,
            },
            "munches happily",
        )),
        KeyCode::Char('p') => Some((
            Stats {
                food: 0.0,
                joy: 15.0,
                energy: 0.0,
            },
            "loves the attention",
        )),
        KeyCode::Char('y') => Some((
            Stats {
                food: -5.0,
                joy: 25.0,
                energy: -10.0,
            },
            "bounces around",
        )),
        _ => None,
    }
}

fn frame_at(clock: Duration) -> &'static str {
    let step = (clock.as_millis() / FRAME_TIME.as_millis()) as usize;
    FRAMES[SEQUENCE[step % SEQUENCE.len()]]
}

fn draw(frame: &mut Frame, pet: &Pet, clock: Duration, message: Option<&str>) {
    let card = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(LAVENDER)
        .title(Line::from(" pet ").centered())
        .title_bottom(Line::styled(HELP, MUTED).centered());
    let area = centered(frame.area(), CARD_WIDTH, CARD_HEIGHT);
    let inside = card.inner(area);
    frame.render_widget(card, area);

    let [stage, status, _, food, joy, energy, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .horizontal_margin(CARD_PADDING)
    .areas(inside);

    let cat = Text::styled(frame_at(clock), Style::new().fg(PINK));
    let area = centered(stage, cat.width() as u16, cat.height() as u16);
    frame.render_widget(cat, area);

    let (message, color) =
        message.map_or_else(|| mood_status(pet.mood()), |message| (message, TEXT));
    let line = Line::styled(format!("{NAME} {message}."), color).centered();
    frame.render_widget(line, status);

    frame.render_widget(bar("Food    ", pet.stats.food, PEACH), food);
    frame.render_widget(bar("Joy     ", pet.stats.joy, PINK), joy);
    frame.render_widget(bar("Energy  ", pet.stats.energy, SKY), energy);
}

fn mood_status(mood: Mood) -> (&'static str, Color) {
    match mood {
        Mood::Hungry => ("is hungry", ROSE),
        Mood::Bored => ("is bored", ROSE),
        Mood::Tired => ("is sleepy", ROSE),
        Mood::Content => ("is doing fine", TEXT),
        Mood::Happy => ("is happy", MINT),
    }
}

fn bar(label: &'static str, value: f32, color: Color) -> LineGauge<'static> {
    let color = if value < pet::LOW { ROSE } else { color };
    LineGauge::default()
        .label(Line::styled(label, TEXT))
        .ratio(f64::from(value / pet::FULL))
        .filled_symbol(THICK_HORIZONTAL)
        .filled_style(color)
        .unfilled_style(MUTED)
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(row);
    cell
}
