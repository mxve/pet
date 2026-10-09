/*!
app:
  holds & modifies state
  inputs and notices
  commands and server replies
  dev cheats
*/

use std::collections::VecDeque;
use std::hash::{BuildHasher, RandomState};
use std::time::Duration;

use pet_core::pet::{self, Activity, Focus, Mood, Pet, Skill};
use pet_core::protocol::Reply;
use pet_core::world::{self, ACTIONS, Clip, Command, Event, Refusal, World};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

use crate::floaters::Floaters;
use crate::online::Remote;
use crate::species::Species;
use crate::theme::YELLOW;

const NOTICE_TIME: Duration = Duration::from_secs(2);
/// batching small increments
const XP_FLOAT_GAP: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Good,
    Bad,
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
    world: Option<World>,
    pub remote: Option<Remote>,
    species: Vec<Species>,
    choice: usize,
    pub screen: Screen,
    /// real time, drives the animations
    pub clock: Duration,
    /// game time, follows the server and runs faster in dev
    time: Duration,
    /// new each start so the background differs
    pub seed: u64,
    pub quit: bool,
    #[cfg(debug_assertions)]
    pub dev: bool,
    adopted: bool,
    acting: Option<(Clip, Duration)>,
    notices: VecDeque<Notice>,
    notice_shown: Duration,
    speed: f32,
    pub floaters: Floaters,
    xp_counted: Option<f32>,
    xp_quiet: Duration,
}

impl App {
    pub fn new(species: Vec<Species>, remote: Option<Remote>, now: Duration) -> App {
        App {
            screen: if remote.is_some() {
                Screen::Home
            } else {
                Screen::Adopt { name: String::new() }
            },
            world: None,
            remote,
            species,
            choice: 0,
            clock: Duration::ZERO,
            time: now,
            seed: RandomState::new().hash_one(0),
            quit: false,
            #[cfg(debug_assertions)]
            dev: false,
            adopted: false,
            acting: None,
            notices: VecDeque::new(),
            notice_shown: Duration::ZERO,
            speed: 1.0,
            floaters: Floaters::default(),
            xp_counted: None,
            xp_quiet: Duration::ZERO,
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
        let action = self.acting.and_then(|(clip, _)| ACTIONS.iter().find(|action| action.clip == clip));
        match (action, self.pet().map(|pet| pet.activity)) {
            (Some(action), _) => Some(action.key),
            (None, Some(Activity::Asleep)) => Some('s'),
            (None, Some(Activity::Training)) => Some('r'),
            (None, Some(Activity::Awake) | None) => None,
        }
    }

    pub fn take_adopted(&mut self) -> bool {
        std::mem::take(&mut self.adopted)
    }

    pub fn say(&mut self, text: &str) {
        self.queue(text, Tone::Plain);
    }

    fn queue(&mut self, text: &str, tone: Tone) {
        let text = text.to_string();
        self.notices.push_back(Notice { text, tone });
    }

    /// jumps the queue, for what just happened
    fn interrupt(&mut self, text: &str, tone: Tone) {
        let text = text.to_string();
        self.notices.push_front(Notice { text, tone });
        self.notice_shown = Duration::ZERO;
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notices.front()
    }

    fn catch_up(&mut self) {
        let events = self.world.as_mut().map(|world| world.catch_up(self.time)).unwrap_or_default();
        self.announce(events);
    }

    /// applied right away, the server answer replaces the guess
    fn send(&mut self, command: Command) {
        let Some(world) = &mut self.world else {
            return;
        };
        let Ok(outcome) = world.apply(command, self.time) else {
            return;
        };
        if let Some(remote) = &self.remote {
            remote.send(command);
        }
        if let Some(clip) = outcome.clip {
            self.acting = Some((clip, Duration::ZERO));
        }
        if let Some(message) = outcome.message {
            self.interrupt(message, Tone::Plain);
        }
        self.announce(outcome.events);
    }

    /// the server time and world win over ours
    fn receive(&mut self) {
        let Some(remote) = &self.remote else {
            return;
        };
        for reply in remote.received() {
            let (server_time, world) = match reply {
                Reply::Synced { server_time, world } => (server_time, world),
                Reply::Done { server_time, world } => (server_time, Some(world)),
                Reply::Refused {
                    server_time,
                    world,
                    reason,
                } => {
                    self.acting = None;
                    let text = match reason {
                        Refusal::Busy => "is still busy.",
                        Refusal::UnknownAction => "does not know how to do that.",
                    };
                    self.interrupt(text, Tone::Bad);
                    (server_time, Some(world))
                }
                Reply::Registered { .. } => continue,
            };
            self.time = server_time;
            let Some(world) = world else {
                continue;
            };
            if let Some(known) = self.species.iter().position(|species| species.name == world.pet().species) {
                self.choice = known;
            }
            self.world = Some(world);
            if matches!(self.screen, Screen::Adopt { .. }) {
                self.screen = Screen::Home;
            }
        }
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
            self.queue(&text, tone);
        }
    }

    /// raw mode eats ctrl+c, so it is handled here
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

    /// the name is measured in terminal columns, wide chars count double
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
                if Span::raw(name.as_str()).width() > pet::NAME_LENGTH {
                    name.pop();
                }
            }
            KeyCode::Enter if pet::valid_name(name.trim()) => {
                let pet = Pet::new(name.trim(), &self.species[self.choice].name);
                self.world = Some(World::new(pet, self.time));
                self.adopted = true;
                self.screen = Screen::Home;
            }
            _ => {}
        }
    }

    fn on_home_key(&mut self, code: KeyCode) {
        if code == KeyCode::Char('q') {
            self.quit = true;
            return;
        }
        let Some(pet) = self.pet() else {
            return;
        };
        let activity = pet.activity;
        let focus = pet.focus;
        match code {
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
            KeyCode::Char(key) if world::action(key).is_some() => self.send(Command::Act(key)),
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

    /// notices wait until the clip is done
    pub fn tick(&mut self, elapsed: Duration) {
        self.clock += elapsed;
        self.time += elapsed.mul_f32(self.speed);
        self.receive();
        self.catch_up();
        self.float_xp(elapsed);
        self.floaters.tick(elapsed);
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

    /// xp can drop when the server corrects us, then it just recounts
    fn float_xp(&mut self, elapsed: Duration) {
        let Some(total) = self.pet().map(Pet::total_xp) else {
            return;
        };
        self.xp_quiet += elapsed;
        let counted = *self.xp_counted.get_or_insert(total);
        let gained = (total - counted).floor();
        if total < counted {
            self.xp_counted = Some(total);
        } else if gained >= 1.0 && self.xp_quiet >= XP_FLOAT_GAP {
            self.floaters.spawn(format!("+{gained} xp"), YELLOW);
            self.xp_counted = Some(counted + gained);
            self.xp_quiet = Duration::ZERO;
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

        /// ctrl+alt is altgr on some keyboards, that stays for typing
        pub(super) fn on_dev_key(&mut self, key: KeyEvent) -> bool {
            let control = key.modifiers.contains(KeyModifiers::CONTROL);
            let alt = key.modifiers.contains(KeyModifiers::ALT);
            if !control || alt {
                return false;
            }
            let cheat = match key.code {
                KeyCode::Char('t') => {
                    let current = SPEEDS.iter().position(|speed| *speed == self.speed);
                    self.speed = SPEEDS[current.map_or(0, |index| (index + 1) % SPEEDS.len())];
                    return true;
                }
                KeyCode::Char('x') => Cheat::Xp,
                KeyCode::Char('l') => Cheat::NextLevel,
                KeyCode::Char('f') => Cheat::Shift(Stats::new(-CHEAT_DRAIN, 0.0, 0.0)),
                KeyCode::Char('p') => Cheat::Shift(Stats::new(0.0, -CHEAT_DRAIN, 0.0)),
                KeyCode::Char('e') => Cheat::Shift(Stats::new(0.0, 0.0, -CHEAT_DRAIN)),
                KeyCode::Char('r') => Cheat::Shift(Stats::new(FULL, FULL, FULL)),
                _ => return true,
            };
            self.send(Command::Cheat(cheat));
            true
        }
    }
}
