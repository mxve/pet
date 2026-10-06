use std::collections::VecDeque;
use std::hash::{BuildHasher, RandomState};
use std::time::Duration;

use pet_core::pet::{Activity, Focus, Mood, Pet, Skill};
use pet_core::world::{ACTIONS, Clip, Command, Event, World};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

use crate::species::Species;

const NAME_WIDTH: usize = 12;
const ONGOING_BLINK: Duration = Duration::from_millis(200);
const NOTICE_TIME: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Good,
}

pub struct Notice {
    pub text: String,
    pub tone: Tone,
}

pub enum Screen {
    Adopt { name: String },
    Home,
    Skills { choice: usize },
}

pub struct App {
    pub world: Option<World>,
    pub species: Vec<Species>,
    pub choice: usize,
    pub screen: Screen,
    pub clock: Duration,
    time: Duration,
    pub seed: u64,
    pub quit: bool,
    #[cfg(debug_assertions)]
    pub dev: bool,
    acting: Option<(Clip, Duration)>,
    notices: VecDeque<Notice>,
    notice_shown: Duration,
    speed: f32,
}

impl App {
    pub fn new(world: Option<World>, species: Vec<Species>, choice: usize, now: Duration) -> App {
        App {
            screen: if world.is_some() {
                Screen::Home
            } else {
                Screen::Adopt { name: String::new() }
            },
            world,
            species,
            choice,
            clock: Duration::ZERO,
            time: now,
            seed: RandomState::new().hash_one(0),
            quit: false,
            #[cfg(debug_assertions)]
            dev: false,
            acting: None,
            notices: VecDeque::new(),
            notice_shown: Duration::ZERO,
            speed: 1.0,
        }
    }

    pub fn pet(&self) -> Option<&Pet> {
        self.world.as_ref().map(World::pet)
    }

    pub fn chosen(&self) -> &Species {
        &self.species[self.choice]
    }

    pub fn frame(&self, pet: &Pet) -> &str {
        let species = self.chosen();
        if let Some((clip, elapsed)) = self.acting {
            return species.animation(clip).frame_at(elapsed);
        }
        let clip = match pet.mood() {
            Mood::Asleep => Clip::Sleep,
            Mood::Training => Clip::Train,
            Mood::Hungry | Mood::Bored | Mood::Tired => Clip::Sad,
            Mood::Content => Clip::Idle,
            Mood::Happy => Clip::Happy,
        };
        species.animation(clip).frame_at(self.clock)
    }

    pub fn ongoing(&self) -> Option<char> {
        let action = self.acting.and_then(|(clip, played)| {
            let action = ACTIONS.iter().find(|action| action.clip == clip)?;
            Some((action.key, played))
        });
        let (key, held) = match (action, self.pet().map(|pet| pet.activity)) {
            (Some(action), _) => action,
            (None, Some(Activity::Asleep)) => ('s', self.clock),
            (None, Some(Activity::Training)) => ('r', self.clock),
            (None, Some(Activity::Awake) | None) => return None,
        };
        let blink = held.as_millis() / ONGOING_BLINK.as_millis();
        blink.is_multiple_of(2).then_some(key)
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notices.front()
    }

    pub fn catch_up(&mut self) {
        let events = self.world.as_mut().map(|world| world.catch_up(self.time)).unwrap_or_default();
        self.announce(events);
    }

    fn send(&mut self, command: Command) {
        let Some(world) = &mut self.world else {
            return;
        };
        let Ok(outcome) = world.apply(command, self.time) else {
            return;
        };
        if let Some(clip) = outcome.clip {
            self.acting = Some((clip, Duration::ZERO));
        }
        if let Some(message) = outcome.message {
            self.notices.push_front(Notice {
                text: message.to_string(),
                tone: Tone::Plain,
            });
            self.notice_shown = Duration::ZERO;
        }
        self.announce(outcome.events);
    }

    fn announce(&mut self, events: Vec<Event>) {
        for event in events {
            let (text, tone) = match event {
                Event::Level(level) => (format!("reached level {level}!"), Tone::Good),
                Event::Skill(skill, level) => (format!("reached {} {level}!", skill.name()), Tone::Good),
                Event::TrainingEnded => ("is worn out from training.".to_string(), Tone::Plain),
            };
            if tone == Tone::Good {
                self.acting = Some((Clip::Cheer, Duration::ZERO));
            }
            self.notices.push_back(Notice { text, tone });
        }
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
            Screen::Skills { choice } => self.on_skills_key(key.code, choice),
        }
    }

    fn on_adopt_key(&mut self, code: KeyCode) {
        let Screen::Adopt { name } = &mut self.screen else {
            return;
        };
        let count = self.species.len();
        match code {
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
                let pet = Pet::new(name.trim(), &self.species[self.choice].name);
                self.world = Some(World::new(pet, self.time));
                self.screen = Screen::Home;
            }
            _ => {}
        }
    }

    fn on_home_key(&mut self, code: KeyCode) {
        let Some(pet) = self.pet() else {
            return;
        };
        let activity = pet.activity;
        let focus = pet.focus;
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('k') => {
                let choice = match focus {
                    Focus::One(skill) => Skill::ALL.iter().position(|each| *each == skill),
                    Focus::All => None,
                };
                self.screen = Screen::Skills {
                    choice: choice.unwrap_or(Skill::ALL.len()),
                };
            }
            KeyCode::Char('s') => self.send(Command::Activity(toggled(activity, Activity::Asleep))),
            KeyCode::Char('r') => self.send(Command::Activity(toggled(activity, Activity::Training))),
            KeyCode::Char(key) if ACTIONS.iter().any(|action| action.key == key) => self.send(Command::Act(key)),
            _ => {}
        }
    }

    fn on_skills_key(&mut self, code: KeyCode, choice: usize) {
        let rows = Skill::ALL.len() + 1;
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Esc | KeyCode::Char('k') => self.screen = Screen::Home,
            KeyCode::Up => {
                self.screen = Screen::Skills {
                    choice: (choice + rows - 1) % rows,
                }
            }
            KeyCode::Down => {
                self.screen = Screen::Skills {
                    choice: (choice + 1) % rows,
                }
            }
            KeyCode::Enter => {
                let focus = Skill::ALL.get(choice).map_or(Focus::All, |&skill| Focus::One(skill));
                self.send(Command::Focus(focus));
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, elapsed: Duration) {
        self.clock += elapsed;
        self.time += elapsed.mul_f32(self.speed);
        self.catch_up();
        self.acting = self
            .acting
            .map(|(clip, played)| (clip, played + elapsed))
            .filter(|(clip, played)| *played < self.chosen().animation(*clip).duration());
        if self.acting.is_none() && !self.notices.is_empty() {
            self.notice_shown += elapsed;
            if self.notice_shown >= NOTICE_TIME {
                self.notices.pop_front();
                self.notice_shown = Duration::ZERO;
            }
        }
    }
}

fn toggled(current: Activity, wanted: Activity) -> Activity {
    if current == wanted { Activity::Awake } else { wanted }
}

#[cfg(debug_assertions)]
mod dev {
    use pet_core::pet::{FULL, Stats};
    use pet_core::world::{Cheat, Command};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::App;

    const SPEED: f32 = 600.0;
    const SPEEDS: [f32; 5] = [1.0, 10.0, 60.0, 600.0, 3600.0];
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
            let by = |food, joy, energy| Stats { food, joy, energy };
            let cheat = match key.code {
                KeyCode::Char('t') => {
                    let current = SPEEDS.iter().position(|speed| *speed == self.speed);
                    self.speed = SPEEDS[current.map_or(0, |index| (index + 1) % SPEEDS.len())];
                    return true;
                }
                KeyCode::Char('x') => Cheat::Xp,
                KeyCode::Char('l') => Cheat::NextLevel,
                KeyCode::Char('f') => Cheat::Shift(by(-CHEAT_DRAIN, 0.0, 0.0)),
                KeyCode::Char('p') => Cheat::Shift(by(0.0, -CHEAT_DRAIN, 0.0)),
                KeyCode::Char('e') => Cheat::Shift(by(0.0, 0.0, -CHEAT_DRAIN)),
                KeyCode::Char('r') => Cheat::Shift(by(FULL, FULL, FULL)),
                _ => return true,
            };
            self.send(Command::Cheat(cheat));
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species;

    #[test]
    fn adopting_needs_a_name_and_any_letter_types() {
        let mut app = App::new(None, species::builtin(), 0, Duration::ZERO);
        let mut press = |codes: &[KeyCode]| {
            for &code in codes {
                app.on_key(KeyEvent::from(code));
            }
        };
        press(&[KeyCode::Left, KeyCode::Right, KeyCode::Char(' '), KeyCode::Enter]);
        press(&[KeyCode::Char('q'), KeyCode::Char('f'), KeyCode::Backspace]);
        press(&[KeyCode::Enter]);
        assert_eq!(app.pet(), Some(&Pet::new("q", &app.species[0].name)));
        assert!(matches!(app.screen, Screen::Home));
        assert!(!app.quit);
    }
}
