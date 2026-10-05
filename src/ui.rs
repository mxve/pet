use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Color;
use ratatui::symbols::line::THICK_HORIZONTAL;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, LineGauge};

use crate::app::{ACTIONS, App, Screen};
use crate::pet::{self, Mood};
use crate::species::{Clip, Species};
use crate::theme::{LAVENDER, MINT, MUTED, PEACH, PINK, ROSE, SKY, TEXT, YELLOW};

const CARD_WIDTH: u16 = 49;
const CARD_HEIGHT: u16 = 14;
const CARD_PADDING: u16 = 3;

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Adopt { name } => draw_adopt(frame, app, name),
        Screen::Home => draw_home(frame, app),
    }
}

fn draw_adopt(frame: &mut Frame, app: &App, name: &str) {
    let species = app.chosen();
    let title = Line::styled(" adopt a pet ", species.color);
    let keys = [("<- ->", "choose"), ("enter", "adopt"), ("esc", "quit")];
    let [stage, choice, _, typed, ..] = card(frame, title, LAVENDER, help(keys));

    let art = species.animation(Clip::Idle).frame_at(app.clock);
    draw_pet(frame, species, art, stage);

    let line = Line::from(vec![
        Span::styled("<  ", MUTED),
        Span::styled(species.name.as_str(), species.color),
        Span::styled("  >", MUTED),
    ]);
    frame.render_widget(line.centered(), choice);

    let line = if name.is_empty() {
        Line::styled("type a name", MUTED)
    } else {
        Line::from(vec![Span::styled(name, TEXT), Span::styled("_", YELLOW)])
    };
    frame.render_widget(line.centered(), typed);
}

fn draw_home(frame: &mut Frame, app: &App) {
    let Some(pet) = &app.pet else {
        return;
    };
    let species = app.chosen();
    let title = Line::styled(
        format!(" {} the {} ", pet.name, species.name),
        species.color,
    );
    let border = if pet.asleep { MUTED } else { LAVENDER };
    let keys = ACTIONS
        .iter()
        .map(|action| (action.key, action.label))
        .chain([('s', "sleep"), ('q', "quit")]);
    let [stage, status, _, food, joy, energy, _] = card(frame, title, border, help(keys));

    draw_pet(frame, species, app.frame(pet), stage);

    let (message, color) = app
        .message()
        .map_or_else(|| mood_status(pet.mood()), |message| (message, TEXT));
    let line = Line::styled(format!("{} {message}.", pet.name), color).centered();
    frame.render_widget(line, status);

    frame.render_widget(bar("Food    ", pet.stats.food, PEACH), food);
    frame.render_widget(bar("Joy     ", pet.stats.joy, PINK), joy);
    frame.render_widget(bar("Energy  ", pet.stats.energy, SKY), energy);
}

fn card(frame: &mut Frame, title: Line, border: Color, help: Line) -> [Rect; 7] {
    let card = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(title.centered())
        .title_bottom(help.centered());
    let area = centered(frame.area(), CARD_WIDTH, CARD_HEIGHT);
    let inside = card.inner(area);
    frame.render_widget(card, area);

    Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .horizontal_margin(CARD_PADDING)
    .areas(inside)
}

fn draw_pet(frame: &mut Frame, species: &Species, art: &str, stage: Rect) {
    let (width, height) = species.size();
    let art = species.paint(art);
    let [area] = Layout::vertical([Constraint::Length(art.height() as u16)])
        .flex(Flex::End)
        .areas(centered(stage, width, height));
    frame.render_widget(art, area);
}

fn help<K: ToString>(keys: impl IntoIterator<Item = (K, &'static str)>) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (index, (key, label)) in keys.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", MUTED));
        }
        spans.extend(hint(key.to_string(), label));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn hint(key: String, label: &'static str) -> [Span<'static>; 3] {
    match label.find(&key) {
        Some(at) => {
            let end = at + key.len();
            [
                Span::styled(&label[..at], MUTED),
                Span::styled(&label[at..end], YELLOW),
                Span::styled(&label[end..], MUTED),
            ]
        }
        None => [
            Span::styled(key, YELLOW),
            Span::raw(" "),
            Span::styled(label, MUTED),
        ],
    }
}

fn mood_status(mood: Mood) -> (&'static str, Color) {
    match mood {
        Mood::Asleep => ("is fast asleep", TEXT),
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
