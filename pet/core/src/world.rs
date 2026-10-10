/*!
world:
  actions and commands
  refusals
  level ups
  catching up
*/

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::pet::{Activity, Focus, Pet, Skill, Stats};

const POINTS_PER_XP: f32 = 2.5;
const ACTION_TIME: Duration = Duration::from_secs(2);
#[cfg(debug_assertions)]
const CHEAT_XP: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clip {
    Idle,
    Happy,
    Sad,
    Eat,
    Pet,
    Play,
    Sleep,
    Train,
    Cheer,
}

pub struct Action {
    pub key: char,
    pub label: &'static str,
    pub clip: Clip,
    pub effect: Stats,
    pub message: &'static str,
    pub busy: Duration,
    pub bond: f32,
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
        busy: ACTION_TIME,
        bond: 1.0,
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
        busy: ACTION_TIME,
        bond: 4.0,
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
        busy: ACTION_TIME,
        bond: 1.0,
    },
];

pub fn action(key: char) -> Option<&'static Action> {
    ACTIONS.iter().find(|action| action.key == key)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Act(char),
    Activity(Activity),
    Focus(Focus),
    #[cfg(debug_assertions)]
    Cheat(Cheat),
}

/// cheat stuff completely excluded from release builds
#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Cheat {
    Xp,
    NextLevel,
    Shift(Stats),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    Busy,
    UnknownAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Level(u32),
    Skill(Skill, u32),
    TrainingEnded,
}

#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub message: Option<&'static str>,
    pub clip: Option<Clip>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pet: Pet,
    last_seen: Duration,
    last_interaction: Duration,
    /// bumped on change so sync can skip known
    revision: u64,
    busy_until: Duration,
}

impl World {
    pub fn new(pet: Pet, now: Duration) -> World {
        World {
            pet,
            last_seen: now,
            last_interaction: now,
            revision: 0,
            busy_until: Duration::ZERO,
        }
    }

    pub fn pet(&self) -> &Pet {
        &self.pet
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn last_seen(&self) -> Duration {
        self.last_seen
    }

    pub fn apply(&mut self, command: Command, now: Duration) -> Result<Outcome, Refusal> {
        let busy = now < self.busy_until;
        let outcome = match command {
            Command::Act(key) => {
                let action = action(key).ok_or(Refusal::UnknownAction)?;
                if busy {
                    return Err(Refusal::Busy);
                }
                self.busy_until = now + action.busy;
                let events = self.changing(|pet| {
                    let points = pet.apply(action.effect, action.bond);
                    pet.earn(points / POINTS_PER_XP);
                });
                Outcome {
                    message: Some(action.message),
                    clip: Some(action.clip),
                    events,
                }
            }
            Command::Activity(_) if busy => return Err(Refusal::Busy),
            Command::Activity(activity) => self.quietly(|pet| pet.activity = activity),
            Command::Focus(focus) => self.quietly(|pet| pet.focus = focus),
            #[cfg(debug_assertions)]
            Command::Cheat(cheat) => self.quietly(|pet| cheated(pet, cheat)),
        };
        self.last_interaction = now;
        self.revision += 1;
        Ok(outcome)
    }

    /// lets pretend time is linear
    pub fn catch_up(&mut self, now: Duration) -> Vec<Event> {
        let away = now.saturating_sub(self.last_seen);
        let idle = self.last_seen.saturating_sub(self.last_interaction);
        self.last_seen = self.last_seen.max(now);
        let was_training = self.pet.activity == Activity::Training;
        let mut events = self.changing(|pet| pet.advance(away, idle));
        if was_training && self.pet.activity != Activity::Training {
            events.push(Event::TrainingEnded);
        }
        events
    }

    fn quietly(&mut self, change: impl FnOnce(&mut Pet)) -> Outcome {
        Outcome {
            events: self.changing(change),
            ..Outcome::default()
        }
    }

    /// trigger level up on any change if needed
    fn changing(&mut self, change: impl FnOnce(&mut Pet)) -> Vec<Event> {
        let before = Levels::of(&self.pet);
        change(&mut self.pet);
        before.risen(&self.pet)
    }
}

struct Levels {
    pet: u32,
    skills: [u32; Skill::ALL.len()],
}

impl Levels {
    fn of(pet: &Pet) -> Levels {
        Levels {
            pet: pet.level().number,
            skills: Skill::ALL.map(|skill| pet.skill(skill).number),
        }
    }

    fn risen(&self, pet: &Pet) -> Vec<Event> {
        let now = Levels::of(pet);
        let pet_level = (now.pet > self.pet).then_some(Event::Level(now.pet));
        let skills = Skill::ALL
            .into_iter()
            .zip(self.skills.into_iter().zip(now.skills))
            .filter(|(_, (before, after))| after > before)
            .map(|(skill, (_, after))| Event::Skill(skill, after));
        pet_level.into_iter().chain(skills).collect()
    }
}

#[cfg(debug_assertions)]
fn cheated(pet: &mut Pet, cheat: Cheat) {
    match cheat {
        Cheat::Xp => pet.earn(CHEAT_XP),
        Cheat::NextLevel => {
            let level = pet.level();
            let skill = pet.tracked();
            *pet.skills.entry(skill).or_default() += (level.needed - level.into) as f32;
        }
        Cheat::Shift(by) => pet.stats = pet.stats.shifted(by),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn actions_wait_for_the_one_playing() {
        let mut world = World::new(Pet::new("Mochi", "Cat"), Duration::ZERO);
        assert!(world.apply(Command::Act('f'), Duration::ZERO).is_ok());
        assert_eq!(world.apply(Command::Act('p'), SECOND), Err(Refusal::Busy));
        assert!(world.apply(Command::Act('p'), 3 * SECOND).is_ok());
    }

    #[test]
    fn care_earns_only_what_helped() {
        let mut world = World::new(Pet::new("Mochi", "Cat"), Duration::ZERO);
        world.pet.focus = Focus::One(Skill::Hitpoints);
        world.pet.stats.food = 0.0;
        world.apply(Command::Act('f'), Duration::ZERO).unwrap();
        let earned = world.pet.xp(Skill::Hitpoints);
        assert!((earned - 12.4).abs() < 0.001);
        world.pet.stats.food = 100.0;
        world.apply(Command::Act('p'), 3 * SECOND).unwrap();
        assert!(world.pet.xp(Skill::Hitpoints) > earned);
        let earned = world.pet.xp(Skill::Hitpoints);
        world.pet.attachment = 100.0;
        world.apply(Command::Act('f'), 6 * SECOND).unwrap();
        assert_eq!(world.pet.xp(Skill::Hitpoints), earned);
    }

    #[test]
    fn level_ups_come_out_as_events_in_order() {
        let mut world = World::new(Pet::new("Mochi", "Cat"), Duration::ZERO);
        world.pet.focus = Focus::One(Skill::Hitpoints);
        let events = world.changing(|pet| {
            pet.earn(300.0);
            pet.focus = Focus::One(Skill::Attack);
            pet.earn(20.0);
        });
        let expected = [Event::Level(5), Event::Skill(Skill::Hitpoints, 15), Event::Skill(Skill::Attack, 1)];
        assert_eq!(events, expected);
    }
}
