/*!
ui:
  card and buttons
  stats and skills
  background noise
  dev overlay
*/

use std::time::Duration;

use pet_core::pet::{self, Activity, Focus, Mood, Pet, Skill};
use pet_core::random::roll;
use pet_core::world::Clip;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear};

use crate::app::{self, App, Click, Screen, Tone};
use crate::bar::Bar;
use crate::species::Species;
use crate::theme::{self, BASE, BUTTON, LAVENDER, MINT, MUTED, PEACH, PINK, ROSE, SKY, SURFACE, TEXT, YELLOW};

const CARD_WIDTH: u16 = 55;
const CARD_HEIGHT: u16 = 16;
const CARD_PADDING: u16 = 3;
const STAT_LABEL_WIDTH: u16 = 9;
const GLOW_STRENGTH: f32 = 0.25;
const POINT_LIFT: f32 = 0.15;
const PILL_GAP: u16 = 1;
const HINT_GAP: u16 = 3;
const SKILL_LIST_TOP: u16 = 2;
const SKILL_ROWS: usize = 8;
const SKILL_NAME_WIDTH: u16 = 13;
const SKILL_LEVEL_WIDTH: u16 = 4;
const SKILL_XP_WIDTH: u16 = 14;
const NOISE: [&str; 4] = [".", "+", "⋆", "✧"];
const NOISE_DENSITY: u64 = 60;
const NOISE_PERIOD: Duration = Duration::from_secs(4);

/// returns what can be clicked and where
pub fn draw(frame: &mut Frame, app: &App) -> Vec<(Rect, Click)> {
    fill(frame, frame.area(), BASE);
    draw_noise(frame, app.seed, app.clock);
    let clickable = match &app.screen {
        Screen::Adopt { name } => draw_adopt(frame, app, name),
        Screen::Home => draw_home(frame, app),
        Screen::Skills { choice } => draw_skills(frame, app, *choice),
    };
    #[cfg(debug_assertions)]
    if app.dev {
        draw_dev_overlay(frame, app);
    }
    clickable
}

fn draw_adopt(frame: &mut Frame, app: &App, name: &str) -> Vec<(Rect, Click)> {
    let species = app.chosen();
    let heading = vec![Span::styled("adopt a pet", species.color)];
    let quit = Click::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    let buttons = [
        ("<- ->", "choose", key(KeyCode::Right)),
        ("enter", "adopt", key(KeyCode::Enter)),
        ("ctrl+c", "quit", quit),
    ];
    let [inside, footer] = card(frame, heading, &ornament(Mood::Happy));
    let mut clickable = draw_buttons(frame, footer, buttons, None, None);
    let [stage, choice, _, typed, ..] = stage_rows(inside);

    let art = species.animation(Clip::Idle).frame_at(app.clock);
    _ = draw_pet(frame, species, art, stage);

    let line = Line::from(vec![
        Span::styled("<  ", MUTED),
        Span::styled(species.name.as_str(), species.color),
        Span::styled("  >", MUTED),
    ]);
    let area = centered(choice, line.width() as u16, 1);
    frame.render_widget(line, area);
    let [left, _, right] = Layout::horizontal([Constraint::Length(3), Constraint::Fill(1), Constraint::Length(3)]).areas(area);
    clickable.extend([(left, key(KeyCode::Left)), (right, key(KeyCode::Right))]);

    let line = if name.is_empty() {
        Line::styled("type a name", MUTED)
    } else {
        Line::from(vec![Span::styled(name, TEXT), Span::styled("_", YELLOW)])
    };
    frame.render_widget(line.centered(), typed);
    clickable
}

fn draw_waiting(frame: &mut Frame) -> Vec<(Rect, Click)> {
    let heading = vec![Span::styled("pet", LAVENDER)];
    let [inside, footer] = card(frame, heading, &ornament(Mood::Happy));
    let clickable = draw_buttons(frame, footer, [('q', "quit", key(KeyCode::Char('q')))], None, None);
    let [_, status, ..] = stage_rows(inside);
    frame.render_widget(Line::styled("waiting for the server...", MUTED).centered(), status);
    clickable
}

fn draw_home(frame: &mut Frame, app: &App) -> Vec<(Rect, Click)> {
    let Some(pet) = app.pet() else {
        return draw_waiting(frame);
    };
    let species = app.chosen();
    let heading = vec![
        Span::styled(format!("{} the {}", pet.name, species.name), species.color),
        Span::styled(format!(" | Lv {}", pet.level().number), TEXT),
    ];
    let [inside, footer] = card(frame, heading, &ornament(pet.mood()));
    let [stage, status, _, food, joy, energy, _, care, _] = stage_rows(inside);
    let mut clickable = Vec::new();
    for (index, (keys, row)) in app::home_buttons().into_iter().zip([care, footer]).enumerate() {
        let pointed = app.pointed().and_then(|(row, column)| (row == index).then_some(column));
        let buttons = keys.into_iter().map(|(letter, label)| (letter, label, key(KeyCode::Char(letter))));
        clickable.extend(draw_buttons(frame, row, buttons, app.ongoing(), pointed));
    }

    let drawn = draw_pet(frame, species, app.frame(pet), stage);
    app.floaters.draw(frame, drawn);

    let (message, color) = match app.notice() {
        Some(notice) => (notice.text.as_str(), tone_color(notice.tone)),
        None => mood_status(pet.mood()),
    };
    let line = Line::styled(format!("{} {message}", pet.name), color).centered();
    frame.render_widget(line, status);

    draw_stat(frame, app.clock, "Food", pet.stats.food, PEACH, food);
    draw_stat(frame, app.clock, "Joy", pet.stats.joy, PINK, joy);
    draw_stat(frame, app.clock, "Energy", pet.stats.energy, SKY, energy);

    let [_, total, focus] = card_rows(frame).map(|row| row.inner(Margin::new(CARD_PADDING + 1, 0)));
    let level = pet.level();
    draw_labeled_bar(frame, app.clock, level.ratio(), format!(" Lv {}", level.number), total);

    if pet.activity == Activity::Training {
        let shown = pet.tracked();
        let skill = pet.skill(shown);
        let label = format!(" {} {}", shown.short_name(), skill.number);
        draw_labeled_bar(frame, app.clock, skill.ratio(), label, focus);
    }
    clickable
}

fn draw_labeled_bar(frame: &mut Frame, clock: Duration, ratio: f32, label: String, row: Rect) {
    let label = Line::styled(label, MUTED);
    let [track, number] = Layout::horizontal([Constraint::Fill(1), Constraint::Length(label.width() as u16)]).areas(row);
    frame.render_widget(Bar::new("xp", ratio).line(track.width, clock), track);
    frame.render_widget(label, number);
}

struct Ornament {
    main: [&'static str; 2],
    small: &'static str,
    color: Color,
}

fn ornament(mood: Mood) -> Ornament {
    let (main, small, color) = match mood {
        Mood::Happy => (["⋆", "⋆"], "·", PINK),
        Mood::Content => (["◦", "◦"], "·", MINT),
        Mood::Training => (["⁺", "⁺"], "·", PEACH),
        Mood::Asleep => (["☾", "☽"], "⋆", LAVENDER),
        Mood::Hungry | Mood::Bored | Mood::Tired => (["°", "°"], "·", ROSE),
    };
    Ornament { main, small, color }
}

/// drawn a row taller each way for rounded edges, returns the inside and the bottom border row
fn card(frame: &mut Frame, heading: Vec<Span>, ornament: &Ornament) -> [Rect; 2] {
    let title = Line::from(heading);
    let card = Block::bordered().border_set(border::EMPTY).title(title.centered());
    let [area, ..] = card_rows(frame);
    let inside = card.inner(area);
    let footer = Rect::new(inside.x, area.bottom().saturating_sub(1), inside.width, 1);
    let padded = Rect::new(area.x, area.y.saturating_sub(1), area.width, area.height + 2).intersection(frame.area());
    draw_surface(frame, padded);
    frame.render_widget(card, area);
    draw_edges(frame, padded, ornament);
    [inside.inner(Margin::new(CARD_PADDING, 0)), footer.intersection(frame.area())]
}

fn stage_rows(inside: Rect) -> [Rect; 9] {
    let mut rows = [Constraint::Length(1); 9];
    rows[0] = Constraint::Fill(1);
    Layout::vertical(rows).areas(inside)
}

fn draw_skills(frame: &mut Frame, app: &App, choice: usize) -> Vec<(Rect, Click)> {
    let Some(pet) = app.pet() else {
        return Vec::new();
    };
    let heading = vec![Span::styled(format!("{}'s skills", pet.name), app.chosen().color)];
    let hints = [
        ("up/down", "move", key(KeyCode::Down)),
        ("enter", "focus", key(KeyCode::Enter)),
        ("esc", "back", key(KeyCode::Esc)),
    ];
    let [inside, footer] = card(frame, heading, &ornament(pet.mood()));
    let mut clickable = draw_hints(frame, footer, hints);
    let rows = Layout::vertical([Constraint::Length(1); SKILL_ROWS]).areas::<SKILL_ROWS>(inside.inner(Margin::new(0, SKILL_LIST_TOP)));

    for (index, skill) in Skill::ALL.into_iter().enumerate() {
        draw_skill_row(frame, app.clock, pet, skill, index == choice, rows[index]);
    }
    let all = choice_label("All skills (slow)", choice == Skill::ALL.len(), pet.focus == Focus::All);
    frame.render_widget(all, rows[Skill::ALL.len()]);
    clickable.extend(
        rows[..=Skill::ALL.len()]
            .iter()
            .enumerate()
            .map(|(index, row)| (*row, Click::Skill(index))),
    );

    let total = Line::styled(format!("Total level {}", pet.total_level()), MUTED);
    frame.render_widget(total.centered(), rows[SKILL_ROWS - 1]);
    clickable
}

fn draw_skill_row(frame: &mut Frame, clock: Duration, pet: &Pet, skill: Skill, chosen: bool, row: Rect) {
    let level = pet.skill(skill);
    let numbers = Line::styled(format!(" {}/{}", level.into, level.needed), MUTED);
    let [name, number, track, xp] = Layout::horizontal([
        Constraint::Length(SKILL_NAME_WIDTH),
        Constraint::Length(SKILL_LEVEL_WIDTH),
        Constraint::Fill(1),
        Constraint::Length(SKILL_XP_WIDTH),
    ])
    .areas(row);
    let label = choice_label(skill.name(), chosen, pet.focus == Focus::One(skill));
    frame.render_widget(label, name);
    frame.render_widget(Line::styled(level.number.to_string(), TEXT), number);
    frame.render_widget(Bar::new(skill.name(), level.ratio()).small().line(track.width, clock), track);
    frame.render_widget(numbers.right_aligned(), xp);
}

fn choice_label(name: &'static str, chosen: bool, focused: bool) -> Line<'static> {
    let cursor = if chosen { Span::styled("> ", YELLOW) } else { Span::raw("  ") };
    Line::from(vec![cursor, Span::styled(name, if focused { PINK } else { TEXT })])
}

#[cfg(debug_assertions)]
fn draw_dev_overlay(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let [stats, keys] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(Rect {
        y: area.bottom().saturating_sub(2),
        height: area.height.min(2),
        ..area
    });
    let clock = app.clock.as_secs_f32();
    let speed = app.speed();
    let line = match app.pet() {
        Some(pet) => format!(
            "dev | {speed}x | {clock:.0}s | food {:.1} joy {:.1} energy {:.1} | {:?} | {} xp {:.2}",
            pet.stats.food,
            pet.stats.joy,
            pet.stats.energy,
            pet.mood(),
            pet.tracked().name(),
            pet.xp(pet.tracked()),
        ),
        None => format!("dev | {speed}x | {clock:.0}s | no pet"),
    };
    frame.render_widget(Line::styled(line, MUTED), stats);
    let help = "ctrl + t speed | x xp | l level | f p e drain | r refill";
    frame.render_widget(Line::styled(help, MUTED), keys);
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

/// every cell twinkles on its own beat
fn time_slot(seed: u64, position: Position, clock: Duration) -> u64 {
    let period = NOISE_PERIOD.as_millis() as u64;
    let offset = roll((seed, position.x, position.y)) % period;
    (clock.as_millis() as u64 + offset) / period
}

fn card_rows(frame: &Frame) -> [Rect; 3] {
    let area = centered(frame.area(), CARD_WIDTH, CARD_HEIGHT + 4);
    let [_, card, _, total, focus] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(CARD_HEIGHT),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    [card, total, focus]
}

fn draw_surface(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    fill(frame, area, BASE);
    fill(frame, area.inner(Margin::new(0, 1)), SURFACE);
}

fn fill(frame: &mut Frame, area: Rect, color: Color) {
    frame.render_widget(Block::new().style(Style::new().bg(color)), area);
}

fn draw_edges(frame: &mut Frame, area: Rect, ornament: &Ornament) {
    if area.width < 6 || area.height < 4 {
        return;
    }
    let (left, right, top, bottom) = (area.left(), area.right() - 1, area.top(), area.bottom() - 1);
    let corner = |x: u16| x <= left + 1 || x >= right - 1;
    let style = Style::new().fg(SURFACE).bg(BASE);
    let inverted = Style::new().fg(BASE).bg(SURFACE);
    let buffer = frame.buffer_mut();
    for x in left..=right {
        buffer[(x, top)].set_symbol(if corner(x) { " " } else { "▄" }).set_style(style);
        buffer[(x, bottom)].set_symbol(if corner(x) { " " } else { "▀" }).set_style(style);
    }
    for (x, y, symbol, style) in [
        (left, top + 1, "▗", style),
        (right, top + 1, "▖", style),
        (left, bottom - 1, "▝", style),
        (right, bottom - 1, "▘", style),
        (left + 2, top, "▂", style),
        (right - 2, top, "▂", style),
        (left + 2, bottom, "▆", inverted),
        (right - 2, bottom, "▆", inverted),
    ] {
        buffer[(x, y)].set_symbol(symbol).set_style(style);
    }
    let [main_left, main_right] = ornament.main;
    let small = ornament.small;
    for (x, y, symbol) in [
        (left + 2, top + 1, main_left),
        (left + 4, top + 1, small),
        (left + 1, top + 2, small),
        (right - 2, top + 1, main_right),
        (right - 4, top + 1, small),
        (right - 1, top + 2, small),
    ] {
        buffer[(x, y)].set_symbol(symbol).set_fg(ornament.color);
    }
}

fn draw_pet(frame: &mut Frame, species: &Species, art: &str, stage: Rect) -> Rect {
    let (width, height) = species.size();
    let art = species.paint(art);
    let [area] = Layout::vertical([Constraint::Length(art.height() as u16)])
        .flex(Flex::End)
        .areas(centered(stage, width, height));
    frame.render_widget(art, area);
    area
}

fn key(code: KeyCode) -> Click {
    Click::Key(KeyEvent::from(code))
}

/// plain text for keys that are no buttons
fn draw_hints(frame: &mut Frame, row: Rect, hints: impl IntoIterator<Item = (&'static str, &'static str, Click)>) -> Vec<(Rect, Click)> {
    let lines = hints.into_iter().map(|(key, label, click)| {
        let line = Line::from(vec![Span::styled(key, YELLOW), Span::styled(format!(" {label}"), MUTED)]);
        (line, click)
    });
    place(frame, row, lines.collect(), HINT_GAP)
}

fn draw_buttons<K: ToString>(
    frame: &mut Frame,
    row: Rect,
    buttons: impl IntoIterator<Item = (K, &'static str, Click)>,
    lit: Option<char>,
    pointed: Option<usize>,
) -> Vec<(Rect, Click)> {
    let pills = buttons
        .into_iter()
        .enumerate()
        .map(|(index, (key, label, click))| (pill(key.to_string(), label, lit, pointed == Some(index)), click));
    place(frame, row, pills.collect(), PILL_GAP)
}

/// centered in the row side by side, returns where each landed
fn place(frame: &mut Frame, row: Rect, items: Vec<(Line<'static>, Click)>, gap: u16) -> Vec<(Rect, Click)> {
    let width = items
        .iter()
        .map(|(line, _)| line.width() as u16 + gap)
        .sum::<u16>()
        .saturating_sub(gap);
    let mut x = centered(row, width, 1).x;
    items
        .into_iter()
        .map(|(line, click)| {
            let width = line.width() as u16;
            let area = Rect::new(x, row.y, width, 1).intersection(row);
            x += width + gap;
            frame.render_widget(line, area);
            (area, click)
        })
        .collect()
}

/// the focus lifts the fill so it still reads on a lit pill
fn pill(key: String, label: &'static str, lit: Option<char>, pointed: bool) -> Line<'static> {
    let glowing = lit.is_some_and(|lit| key == lit.to_string());
    let (fill, word) = if glowing {
        (theme::mix(SURFACE, PINK, GLOW_STRENGTH), PINK)
    } else {
        (BUTTON, MUTED)
    };
    let (fill, word) = if pointed {
        (theme::mix(fill, TEXT, POINT_LIFT), TEXT)
    } else {
        (fill, word)
    };
    let inside = Style::new().bg(fill);
    let mut spans = vec![Span::styled("▗", fill), Span::styled(" ", inside)];
    spans.extend(hint(key, label, inside.fg(word), inside.fg(YELLOW)));
    spans.extend([Span::styled(" ", inside), Span::styled("▘", fill)]);
    Line::from(spans)
}

fn hint(key: String, label: &'static str, word: Style, letter: Style) -> [Span<'static>; 3] {
    match label.find(&key) {
        Some(at) => {
            let end = at + key.len();
            [
                Span::styled(&label[..at], word),
                Span::styled(&label[at..end], letter),
                Span::styled(&label[end..], word),
            ]
        }
        None => [Span::styled(key, letter), Span::styled(" ", word), Span::styled(label, word)],
    }
}

fn tone_color(tone: Tone) -> Color {
    match tone {
        Tone::Plain => TEXT,
        Tone::Good => MINT,
        Tone::Bad => ROSE,
    }
}

fn mood_status(mood: Mood) -> (&'static str, Color) {
    match mood {
        Mood::Asleep => ("is fast asleep.", TEXT),
        Mood::Training => ("is training hard.", TEXT),
        Mood::Hungry => ("is hungry.", ROSE),
        Mood::Bored => ("is bored.", ROSE),
        Mood::Tired => ("is sleepy.", ROSE),
        Mood::Content => ("is doing fine.", TEXT),
        Mood::Happy => ("is happy~", MINT),
    }
}

fn draw_stat(frame: &mut Frame, clock: Duration, label: &'static str, value: f32, color: Color, row: Rect) {
    let color = if value < pet::LOW { ROSE } else { color };
    let [name, track] = Layout::horizontal([Constraint::Length(STAT_LABEL_WIDTH), Constraint::Fill(1)]).areas(row);
    frame.render_widget(Line::styled(label, TEXT), name);
    frame.render_widget(
        Bar::new(label, value / pet::FULL).small().fading_to(color).line(track.width, clock),
        track,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center).areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center).areas(row);
    cell
}
