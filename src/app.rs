use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::pet::{Mood, Pet, Stats};
use crate::species::{Clip, Species};

pub struct Action {
    pub key: char,
    pub label: &'static str,
    pub clip: Clip,
    pub effect: Stats,
    pub message: &'static str,
}

pub const ACTIONS: &[Action] = &[
    Action {
        key: 'f',
        label: "feed",
        clip: Clip::Eat,
        effect: Stats {
            food: 30.0,
            joy: 2.0,
            energy: 0.0,
        },
        message: "munches happily",
    },
    Action {
        key: 'p',
        label: "pet",
        clip: Clip::Pet,
        effect: Stats {
            food: 0.0,
            joy: 15.0,
            energy: 0.0,
        },
        message: "loves the attention",
    },
    Action {
        key: 'y',
        label: "play",
        clip: Clip::Play,
        effect: Stats {
            food: -5.0,
            joy: 25.0,
            energy: -10.0,
        },
        message: "bounces around",
    },
];

pub struct App {
    pub pet: Pet,
    pub species: Species,
    pub clock: Duration,
    pub quit: bool,
    acting: Option<(&'static Action, Duration)>,
    speed: f32,
}

impl App {
    pub fn new(species: Species, speed: f32) -> App {
        App {
            pet: Pet::new(),
            species,
            clock: Duration::ZERO,
            quit: false,
            acting: None,
            speed,
        }
    }

    pub fn frame(&self) -> &str {
        if let Some((action, elapsed)) = self.acting {
            return self.species.animation(action.clip).frame_at(elapsed);
        }
        let clip = match self.pet.mood() {
            Mood::Asleep => Clip::Sleep,
            Mood::Hungry | Mood::Bored | Mood::Tired => Clip::Sad,
            Mood::Content => Clip::Idle,
            Mood::Happy => Clip::Happy,
        };
        self.species.animation(clip).frame_at(self.clock)
    }

    pub fn message(&self) -> Option<&'static str> {
        self.acting.map(|(action, _)| action.message)
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if is_quit(key) {
            self.quit = true;
        } else if key.code == KeyCode::Char('s') {
            self.pet.asleep = !self.pet.asleep;
            self.acting = None;
        } else if let KeyCode::Char(character) = key.code
            && let Some(action) = ACTIONS.iter().find(|action| action.key == character)
        {
            self.pet.apply(action.effect);
            self.acting = Some((action, Duration::ZERO));
        }
    }

    pub fn tick(&mut self, elapsed: Duration) {
        self.clock += elapsed;
        self.pet.tick(elapsed.mul_f32(self.speed));
        self.acting = self
            .acting
            .map(|(action, played)| (action, played + elapsed))
            .filter(|(action, played)| *played < self.species.animation(action.clip).duration());
    }
}

fn is_quit(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species;

    #[test]
    fn an_action_plays_once_then_ends() {
        let mut app = App::new(species::builtin().remove(0), 1.0);
        app.on_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(app.message(), Some("munches happily"));
        app.tick(Duration::from_secs(60));
        assert_eq!(app.message(), None);
    }
}
