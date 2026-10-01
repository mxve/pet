use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Color;
use ratatui::symbols::line::THICK_HORIZONTAL;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, LineGauge};

use crate::app::{ACTIONS, App};
use crate::pet::{self, Mood};
use crate::theme::{LAVENDER, MINT, MUTED, PEACH, PINK, ROSE, SKY, TEXT, YELLOW};

const CARD_WIDTH: u16 = 49;
const CARD_HEIGHT: u16 = 13;
const CARD_PADDING: u16 = 3;

const NAME: &str = "Mochi";

pub fn draw(frame: &mut Frame, app: &App) {
    let card = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(LAVENDER)
        .title(Line::from(" pet ").centered())
        .title_bottom(help().centered());
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

    let species = &app.species;
    let (width, height) = species.size();
    let art = species.paint(app.frame());
    let [art_area] = Layout::vertical([Constraint::Length(art.height() as u16)])
        .flex(Flex::End)
        .areas(centered(stage, width, height));
    frame.render_widget(art, art_area);

    let (message, color) = app
        .message()
        .map_or_else(|| mood_status(app.pet.mood()), |message| (message, TEXT));
    let line = Line::styled(format!("{NAME} {message}."), color).centered();
    frame.render_widget(line, status);

    let stats = app.pet.stats;
    frame.render_widget(bar("Food    ", stats.food, PEACH), food);
    frame.render_widget(bar("Joy     ", stats.joy, PINK), joy);
    frame.render_widget(bar("Energy  ", stats.energy, SKY), energy);
}

fn help() -> Line<'static> {
    let keys = ACTIONS
        .iter()
        .map(|action| (action.key, action.label))
        .chain([('q', "quit")]);
    let mut spans = vec![Span::raw(" ")];
    for (index, (key, label)) in keys.enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", MUTED));
        }
        spans.push(Span::styled(key.to_string(), YELLOW));
        spans.push(Span::styled(format!(" {label}"), MUTED));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
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
