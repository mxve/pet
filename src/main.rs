use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};
use ratatui::{DefaultTerminal, Frame};

const TICK: Duration = Duration::from_millis(100);
const CARD_WIDTH: u16 = 49;
const CARD_HEIGHT: u16 = 13;

fn main() -> io::Result<()> {
    let result = run(&mut ratatui::init());
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    loop {
        terminal.draw(draw)?;
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

fn draw(frame: &mut Frame) {
    let card = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(" pet ").centered());
    frame.render_widget(card, centered(frame.area()));
}

fn centered(area: Rect) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(CARD_HEIGHT)])
        .flex(Flex::Center)
        .areas(area);
    let [card] = Layout::horizontal([Constraint::Length(CARD_WIDTH)])
        .flex(Flex::Center)
        .areas(row);
    card
}
