use std::hash::{BuildHasher, RandomState};
use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

use crate::pet::{Mood, Pet, Stats};
use crate::species::{Clip, Species};

const NAME_WIDTH: usize = 12;
const POINTS_PER_XP: f32 = 5.0;
const ONGOING_BLINK: Duration = Duration::from_millis(200);

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
    pub seed: u64,
    pub quit: bool,
    #[cfg(debug_assertions)]
    pub dev: bool,
    acting: Option<(&'static Action, Duration)>,
    speed: f32,
}

impl App {
    pub fn new(pet: Option<Pet>, species: Vec<Species>, choice: usize) -> App {
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
            seed: RandomState::new().hash_one(0),
            quit: false,
            #[cfg(debug_assertions)]
            dev: false,
            acting: None,
            speed: 1.0,
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

    pub fn ongoing(&self) -> Option<char> {
        let (key, held) = match (self.acting, &self.pet) {
            (Some((action, played)), _) => (action.key, played),
            (None, Some(pet)) if pet.asleep => ('s', self.clock),
            _ => return None,
        };
        let blink = held.as_millis() / ONGOING_BLINK.as_millis();
        blink.is_multiple_of(2).then_some(key)
    }

    pub fn message(&self) -> Option<&'static str> {
        self.acting.map(|(action, _)| action.message)
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        #[cfg(debug_assertions)]
        if self.dev && self.on_dev_key(key) {
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
            _ if self.acting.is_some() => {}
            KeyCode::Char('s') => {
                pet.asleep = !pet.asleep;
            }
            KeyCode::Char(character) => {
                if let Some(action) = ACTIONS.iter().find(|action| action.key == character) {
                    let points = pet.apply(action.effect);
                    pet.earn(points / POINTS_PER_XP);
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

#[cfg(debug_assertions)]
mod dev {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::App;
    use crate::pet::{FULL, Stats};

    const SPEED: f32 = 600.0;
    const SPEEDS: [f32; 5] = [1.0, 10.0, 60.0, 600.0, 3600.0];
    const CHEAT_XP: f32 = 10.0;
    const CHEAT_DRAIN: f32 = 25.0;

    impl App {
        pub fn start_dev(&mut self) {
            self.dev = true;
            self.speed = SPEED;
        }

        pub fn speed(&self) -> f32 {
            self.speed
        }

        pub(super) fn on_dev_key(&mut self, key: KeyEvent) -> bool {
            let control = key.modifiers.contains(KeyModifiers::CONTROL);
            let alt = key.modifiers.contains(KeyModifiers::ALT);
            if !control || alt {
                return false;
            }
            if key.code == KeyCode::Char('t') {
                let current = SPEEDS.iter().position(|speed| *speed == self.speed);
                self.speed = SPEEDS[current.map_or(0, |index| (index + 1) % SPEEDS.len())];
                return true;
            }
            let Some(pet) = &mut self.pet else {
                return true;
            };
            let by = |food, joy, energy| Stats { food, joy, energy };
            match key.code {
                KeyCode::Char('x') => pet.earn(CHEAT_XP),
                KeyCode::Char('l') => {
                    let level = pet.skill(pet.tracked());
                    pet.earn((level.needed - level.into) as f32);
                }
                KeyCode::Char('f') => pet.stats = pet.stats.shifted(by(-CHEAT_DRAIN, 0.0, 0.0)),
                KeyCode::Char('p') => pet.stats = pet.stats.shifted(by(0.0, -CHEAT_DRAIN, 0.0)),
                KeyCode::Char('e') => pet.stats = pet.stats.shifted(by(0.0, 0.0, -CHEAT_DRAIN)),
                KeyCode::Char('r') => pet.stats = pet.stats.shifted(by(FULL, FULL, FULL)),
                _ => {}
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::Skill;
    use crate::species;

    #[test]
    fn an_action_plays_once_then_ends() {
        let species = species::builtin();
        let pet = Pet::new("Mochi", &species[0].name);
        let mut app = App::new(Some(pet), species, 0);
        app.on_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(app.message(), Some("munches happily."));
        app.tick(Duration::from_secs(60));
        assert_eq!(app.message(), None);
    }

    #[test]
    fn actions_wait_for_the_one_playing() {
        let species = species::builtin();
        let pet = Pet::new("Mochi", &species[0].name);
        let mut app = App::new(Some(pet), species, 0);
        app.on_key(KeyEvent::from(KeyCode::Char('f')));
        app.on_key(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(app.message(), Some("munches happily."));
        app.tick(Duration::from_secs(60));
        app.on_key(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(app.message(), Some("loves the attention."));
    }

    #[test]
    fn only_care_that_helps_earns_xp() {
        let species = species::builtin();
        let mut pet = Pet::new("Mochi", &species[0].name);
        pet.stats.food = 0.0;
        let mut app = App::new(Some(pet), species, 0);
        let feed = |app: &mut App| {
            app.on_key(KeyEvent::from(KeyCode::Char('f')));
            app.tick(Duration::from_secs(60));
            app.pet.as_ref().unwrap().xp(Skill::Hitpoints)
        };
        assert_eq!(feed(&mut app), 6.0);
        for _ in 0..5 {
            feed(&mut app);
        }
        let full = feed(&mut app);
        assert!(feed(&mut app) - full < 0.1);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn cheats_only_work_in_dev_mode() {
        let species = species::builtin();
        let pet = Pet::new("Mochi", &species[0].name);
        let mut app = App::new(Some(pet), species, 0);
        let level_up = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        app.on_key(level_up);
        assert_eq!(app.pet.as_ref().unwrap().level(), 5);
        app.start_dev();
        app.on_key(level_up);
        assert_eq!(app.pet.as_ref().unwrap().level(), 6);
    }

    #[test]
    fn adopting_needs_a_name_and_any_letter_types() {
        let mut app = App::new(None, species::builtin(), 0);
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
