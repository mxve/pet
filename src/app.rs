use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

use crate::pet::{Mood, Pet, Stats};
use crate::species::{Clip, Species};

const NAME_WIDTH: usize = 12;

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
        message: "munches happily.",
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
        message: "loves the attention.",
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
        message: "bounces around.",
    },
];

pub enum Screen {
    Adopt { name: String },
    Home,
}

pub struct App {
    pub pet: Option<Pet>,
    pub species: Vec<Species>,
    pub choice: usize,
    pub screen: Screen,
    pub clock: Duration,
    pub quit: bool,
    acting: Option<(&'static Action, Duration)>,
    speed: f32,
}

impl App {
    pub fn new(pet: Option<Pet>, species: Vec<Species>, choice: usize, speed: f32) -> App {
        App {
            screen: if pet.is_some() {
                Screen::Home
            } else {
                Screen::Adopt {
                    name: String::new(),
                }
            },
            pet,
            species,
            choice,
            clock: Duration::ZERO,
            quit: false,
            acting: None,
            speed,
        }
    }

    pub fn chosen(&self) -> &Species {
        &self.species[self.choice]
    }

    pub fn frame(&self, pet: &Pet) -> &str {
        let species = self.chosen();
        if let Some((action, elapsed)) = self.acting {
            return species.animation(action.clip).frame_at(elapsed);
        }
        let clip = match pet.mood() {
            Mood::Asleep => Clip::Sleep,
            Mood::Hungry | Mood::Bored | Mood::Tired => Clip::Sad,
            Mood::Content => Clip::Idle,
            Mood::Happy => Clip::Happy,
        };
        species.animation(clip).frame_at(self.clock)
    }

    pub fn message(&self) -> Option<&'static str> {
        self.acting.map(|(action, _)| action.message)
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        match self.screen {
            Screen::Adopt { .. } => self.on_adopt_key(key.code),
            Screen::Home => self.on_home_key(key.code),
        }
    }

    fn on_adopt_key(&mut self, code: KeyCode) {
        let Screen::Adopt { name } = &mut self.screen else {
            return;
        };
        let count = self.species.len();
        match code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Left => self.choice = (self.choice + count - 1) % count,
            KeyCode::Right => self.choice = (self.choice + 1) % count,
            KeyCode::Backspace => _ = name.pop(),
            KeyCode::Char(character) => {
                name.push(character);
                if Span::raw(name.as_str()).width() > NAME_WIDTH {
                    name.pop();
                }
            }
            KeyCode::Enter if !name.trim().is_empty() => {
                self.pet = Some(Pet::new(name.trim(), &self.species[self.choice].name));
                self.screen = Screen::Home;
            }
            _ => {}
        }
    }

    fn on_home_key(&mut self, code: KeyCode) {
        let Some(pet) = &mut self.pet else {
            return;
        };
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('s') => {
                pet.asleep = !pet.asleep;
                self.acting = None;
            }
            KeyCode::Char(character) => {
                if let Some(action) = ACTIONS.iter().find(|action| action.key == character) {
                    pet.apply(action.effect);
                    self.acting = Some((action, Duration::ZERO));
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, elapsed: Duration) {
        self.clock += elapsed;
        if let Some(pet) = &mut self.pet {
            pet.tick(elapsed.mul_f32(self.speed));
        }
        self.acting = self
            .acting
            .map(|(action, played)| (action, played + elapsed))
            .filter(|(action, played)| *played < self.chosen().animation(action.clip).duration());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species;

    #[test]
    fn an_action_plays_once_then_ends() {
        let species = species::builtin();
        let pet = Pet::new("Mochi", &species[0].name);
        let mut app = App::new(Some(pet), species, 0, 1.0);
        app.on_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(app.message(), Some("munches happily."));
        app.tick(Duration::from_secs(60));
        assert_eq!(app.message(), None);
    }

    #[test]
    fn adopting_needs_a_name_and_any_letter_types() {
        let mut app = App::new(None, species::builtin(), 0, 1.0);
        let mut press = |codes: &[KeyCode]| {
            for &code in codes {
                app.on_key(KeyEvent::from(code));
            }
        };
        press(&[
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Char(' '),
            KeyCode::Enter,
        ]);
        press(&[KeyCode::Char('q'), KeyCode::Char('f'), KeyCode::Backspace]);
        press(&[KeyCode::Enter]);
        assert_eq!(app.pet, Some(Pet::new("q", &app.species[0].name)));
        assert!(matches!(app.screen, Screen::Home));
        assert!(!app.quit);
    }
}
