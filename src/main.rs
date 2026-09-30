use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType};
use ratatui::{DefaultTerminal, Frame};

const TICK: Duration = Duration::from_millis(100);
const CARD_WIDTH: u16 = 49;
const CARD_HEIGHT: u16 = 13;

const PINK: Color = Color::Rgb(0xf5, 0xc2, 0xe7);
const LAVENDER: Color = Color::Rgb(0xb4, 0xbe, 0xfe);

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
    let started = Instant::now();
    loop {
        let clock = started.elapsed();
        terminal.draw(|frame| draw(frame, clock))?;
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && is_quit(key)
        {
            return Ok(());
        }
    }
}

fn is_quit(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

fn frame_at(clock: Duration) -> &'static str {
    let step = (clock.as_millis() / FRAME_TIME.as_millis()) as usize;
    FRAMES[SEQUENCE[step % SEQUENCE.len()]]
}

fn draw(frame: &mut Frame, clock: Duration) {
    let card = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(LAVENDER)
        .title(Line::from(" pet ").centered());
    let area = centered(frame.area(), CARD_WIDTH, CARD_HEIGHT);
    let inside = card.inner(area);
    frame.render_widget(card, area);

    let cat = Text::styled(frame_at(clock), Style::new().fg(PINK));
    let area = centered(inside, cat.width() as u16, cat.height() as u16);
    frame.render_widget(cat, area);
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
