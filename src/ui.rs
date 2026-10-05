use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Margin, Position, Rect};
use ratatui::style::Color;
use ratatui::symbols::line::THICK_HORIZONTAL;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, LineGauge};

use crate::app::{ACTIONS, App, Screen};
use crate::pet::{self, Mood};
use crate::species::{Clip, Species};
use crate::theme::{LAVENDER, MINT, MUTED, PEACH, PINK, ROSE, SKY, TEXT, YELLOW};

const CARD_WIDTH: u16 = 49;
const CARD_HEIGHT: u16 = 14;
const CARD_PADDING: u16 = 3;
const NOISE: [&str; 4] = [".", "+", "⋆", "✧"];
const NOISE_DENSITY: u64 = 60;
const NOISE_PERIOD: Duration = Duration::from_secs(4);

struct Decoration {
    title: [&'static str; 2],
    ornament: [&'static str; 2],
    color: Color,
}

const ADOPT: Decoration = Decoration {
    title: ["✧", "✧"],
    ornament: ["✧", "⋆"],
    color: PINK,
};

pub fn draw(frame: &mut Frame, app: &App) {
    draw_noise(frame, app.seed, app.clock);
    match &app.screen {
        Screen::Adopt { name } => draw_adopt(frame, app, name),
        Screen::Home => draw_home(frame, app),
    }
}

fn draw_adopt(frame: &mut Frame, app: &App, name: &str) {
    let species = app.chosen();
    let heading = Span::styled("adopt a pet", species.color);
    let keys = [("<- ->", "choose"), ("enter", "adopt"), ("esc", "quit")];
    let [stage, choice, _, typed, ..] = card(frame, heading, LAVENDER, &ADOPT, help(keys, None));

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
    let heading = Span::styled(format!("{} the {}", pet.name, species.name), species.color);
    let border = if pet.asleep { MUTED } else { LAVENDER };
    let keys = ACTIONS
        .iter()
        .map(|action| (action.key, action.label))
        .chain([('s', "sleep"), ('q', "quit")]);
    let [stage, status, _, food, joy, energy, _] = card(
        frame,
        heading,
        border,
        &decoration(pet.mood()),
        help(keys, app.ongoing()),
    );

    draw_pet(frame, species, app.frame(pet), stage);

    let (message, color) = app
        .message()
        .map_or_else(|| mood_status(pet.mood()), |message| (message, TEXT));
    let line = Line::styled(format!("{} {message}", pet.name), color).centered();
    frame.render_widget(line, status);

    frame.render_widget(bar("Food    ", pet.stats.food, PEACH), food);
    frame.render_widget(bar("Joy     ", pet.stats.joy, PINK), joy);
    frame.render_widget(bar("Energy  ", pet.stats.energy, SKY), energy);
}

fn card(
    frame: &mut Frame,
    heading: Span,
    border: Color,
    decoration: &Decoration,
    help: Line,
) -> [Rect; 7] {
    let color = decoration.color;
    let [left, right] = decoration.title;
    let title = Line::from(vec![
        Span::styled(format!(" {left} "), color),
        heading,
        Span::styled(format!(" {right} "), color),
    ]);
    let card = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(title.centered())
        .title_bottom(help.centered());
    let area = centered(frame.area(), CARD_WIDTH, CARD_HEIGHT);
    let inside = card.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(card, area);
    draw_ornaments(frame, decoration, inside);

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

fn draw_noise(frame: &mut Frame, seed: u64, clock: Duration) {
    for position in frame.area().positions() {
        if let Some(glyph) = noise_at(seed, position, clock) {
            frame.buffer_mut()[position].set_symbol(glyph).set_fg(MUTED);
        }
    }
}

fn noise_at(seed: u64, position: Position, clock: Duration) -> Option<&'static str> {
    let slot = time_slot(seed, position, clock);
    let dice = roll((seed, position.x, position.y, slot));
    let glyph = NOISE[(dice / NOISE_DENSITY) as usize % NOISE.len()];
    dice.is_multiple_of(NOISE_DENSITY).then_some(glyph)
}

fn time_slot(seed: u64, position: Position, clock: Duration) -> u64 {
    let period = NOISE_PERIOD.as_millis() as u64;
    let offset = roll((seed, position.x, position.y)) % period;
    (clock.as_millis() as u64 + offset) / period
}

fn roll(key: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

fn draw_ornaments(frame: &mut Frame, decoration: &Decoration, inside: Rect) {
    let [big, small] = decoration.ornament;
    let area = inside.inner(Margin::new(1, 0));
    let ornaments = Text::from(vec![
        Line::from(format!("{big} {small}")),
        Line::from(small),
    ]);
    let mirrored = Text::from(vec![
        Line::from(format!("{small} {big}")).right_aligned(),
        Line::from(small).right_aligned(),
    ]);
    frame.render_widget(ornaments.style(decoration.color), area);
    frame.render_widget(mirrored.style(decoration.color), area);
}

fn draw_pet(frame: &mut Frame, species: &Species, art: &str, stage: Rect) {
    let (width, height) = species.size();
    let art = species.paint(art);
    let [area] = Layout::vertical([Constraint::Length(art.height() as u16)])
        .flex(Flex::End)
        .areas(centered(stage, width, height));
    frame.render_widget(art, area);
}

fn help<K: ToString>(
    keys: impl IntoIterator<Item = (K, &'static str)>,
    lit: Option<char>,
) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (index, (key, label)) in keys.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", MUTED));
        }
        let key = key.to_string();
        let glowing = lit.is_some_and(|lit| key == lit.to_string());
        spans.extend(hint(key, label, glowing));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn hint(key: String, label: &'static str, glowing: bool) -> [Span<'static>; 3] {
    let word = if glowing { PINK } else { MUTED };
    match label.find(&key) {
        Some(at) => {
            let end = at + key.len();
            [
                Span::styled(&label[..at], word),
                Span::styled(&label[at..end], YELLOW),
                Span::styled(&label[end..], word),
            ]
        }
        None => [
            Span::styled(key, YELLOW),
            Span::raw(" "),
            Span::styled(label, MUTED),
        ],
    }
}

fn decoration(mood: Mood) -> Decoration {
    match mood {
        Mood::Happy => Decoration {
            title: ["♥", "♥"],
            ornament: ["♥", "✧"],
            color: PINK,
        },
        Mood::Content => Decoration {
            title: ["✿", "✿"],
            ornament: ["✿", "⋆"],
            color: MINT,
        },
        Mood::Asleep => Decoration {
            title: ["☾", "⋆"],
            ornament: ["⋆", "."],
            color: LAVENDER,
        },
        Mood::Hungry | Mood::Bored | Mood::Tired => Decoration {
            title: ["♡", "♡"],
            ornament: ["♡", "."],
            color: ROSE,
        },
    }
}

fn mood_status(mood: Mood) -> (&'static str, Color) {
    match mood {
        Mood::Asleep => ("is fast asleep.", TEXT),
        Mood::Hungry => ("is hungry.", ROSE),
        Mood::Bored => ("is bored.", ROSE),
        Mood::Tired => ("is sleepy.", ROSE),
        Mood::Content => ("is doing fine.", TEXT),
        Mood::Happy => ("is happy~", MINT),
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
