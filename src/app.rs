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

pub enum Screen {
    Adopt,
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
                Screen::Adopt
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
            Screen::Adopt => self.on_adopt_key(key.code),
            Screen::Home => self.on_home_key(key.code),
        }
    }

    fn on_adopt_key(&mut self, code: KeyCode) {
        let count = self.species.len();
        match code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Left => self.choice = (self.choice + count - 1) % count,
            KeyCode::Right => self.choice = (self.choice + 1) % count,
            KeyCode::Enter => {
                let name = &self.chosen().name;
                self.pet = Some(Pet::new(name, name));
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
        let pet = Pet::new("Mochi", "Cat");
        let mut app = App::new(Some(pet), species::builtin(), 0, 1.0);
        app.on_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(app.message(), Some("munches happily"));
        app.tick(Duration::from_secs(60));
        assert_eq!(app.message(), None);
    }

    #[test]
    fn enter_adopts_the_species_on_screen() {
        let mut app = App::new(None, species::builtin(), 0, 1.0);
        for code in [KeyCode::Left, KeyCode::Right, KeyCode::Enter] {
            app.on_key(KeyEvent::from(code));
        }
        assert_eq!(app.pet, Some(Pet::new("Cat", "Cat")));
        assert!(matches!(app.screen, Screen::Home));
    }
}
