use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::pet::{Mood, Pet, Stats};
use crate::species::{Clip, Species};

const MESSAGE_TIME: Duration = Duration::from_millis(1500);

pub struct App {
    pub pet: Pet,
    pub species: Species,
    pub clock: Duration,
    pub quit: bool,
    acting: Option<(&'static str, Duration)>,
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
        let clip = match self.pet.mood() {
            Mood::Hungry | Mood::Bored | Mood::Tired => Clip::Sad,
            Mood::Content => Clip::Idle,
            Mood::Happy => Clip::Happy,
        };
        self.species.animation(clip).frame_at(self.clock)
    }

    pub fn message(&self) -> Option<&'static str> {
        self.acting.map(|(message, _)| message)
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if is_quit(key) {
            self.quit = true;
        } else if let Some((effect, message)) = action(key.code) {
            self.pet.apply(effect);
            self.acting = Some((message, MESSAGE_TIME));
        }
    }

    pub fn tick(&mut self, elapsed: Duration) {
        self.clock += elapsed;
        self.pet.tick(elapsed.mul_f32(self.speed));
        self.acting = self
            .acting
            .and_then(|(message, left)| Some((message, left.checked_sub(elapsed)?)));
    }
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
