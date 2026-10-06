use std::time::Duration;

use serde::Deserialize;

use crate::pet::{Activity, Focus, Pet, Skill, Stats};

const POINTS_PER_XP: f32 = 5.0;
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
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Act(char),
    Activity(Activity),
    Focus(Focus),
    #[cfg(debug_assertions)]
    Cheat(Cheat),
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cheat {
    Xp,
    NextLevel,
    Shift(Stats),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub struct World {
    pet: Pet,
    busy_until: Duration,
    revision: u64,
}

impl World {
    pub fn new(pet: Pet) -> World {
        World {
            pet,
            busy_until: Duration::ZERO,
            revision: 0,
        }
    }

    pub fn pet(&self) -> &Pet {
        &self.pet
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn stamp(&mut self, now: u64) {
        self.pet.last_seen = now;
    }

    pub fn apply(&mut self, command: Command, now: Duration) -> Result<Outcome, Refusal> {
        let busy = now < self.busy_until;
        let outcome = match command {
            Command::Act(key) => {
                let action = ACTIONS.iter().find(|action| action.key == key).ok_or(Refusal::UnknownAction)?;
                if busy {
                    return Err(Refusal::Busy);
                }
                self.busy_until = now + action.busy;
                let events = self.changing(|pet| {
                    let points = pet.apply(action.effect);
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
        self.revision += 1;
        Ok(outcome)
    }

    pub fn tick(&mut self, elapsed: Duration) -> Vec<Event> {
        self.passing(|pet| pet.tick(elapsed))
    }

    pub fn advance(&mut self, elapsed: Duration) -> Vec<Event> {
        self.passing(|pet| pet.advance(elapsed))
    }

    fn passing(&mut self, time: impl FnOnce(&mut Pet)) -> Vec<Event> {
        let was_training = self.pet.activity == Activity::Training;
        let mut events = self.changing(time);
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
            pet.earn((level.needed - level.into) as f32);
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
        let mut world = World::new(Pet::new("Mochi", "Cat"));
        assert!(world.apply(Command::Act('f'), Duration::ZERO).is_ok());
        assert_eq!(world.apply(Command::Act('p'), SECOND), Err(Refusal::Busy));
        assert!(world.apply(Command::Act('p'), 3 * SECOND).is_ok());
    }

    #[test]
    fn only_care_that_helps_earns_xp() {
        let mut world = World::new(Pet::new("Mochi", "Cat"));
        world.pet.stats.food = 0.0;
        world.apply(Command::Act('f'), Duration::ZERO).unwrap();
        assert_eq!(world.pet.xp(Skill::Hitpoints), 6.0);
        world.pet.stats.food = 100.0;
        world.apply(Command::Act('f'), 3 * SECOND).unwrap();
        assert_eq!(world.pet.xp(Skill::Hitpoints), 6.0);
    }

    #[test]
    fn level_ups_come_out_as_events_in_order() {
        let mut world = World::new(Pet::new("Mochi", "Cat"));
        let events = world.changing(|pet| {
            pet.earn(300.0);
            pet.focus = Focus::One(Skill::Attack);
            pet.earn(20.0);
        });
        let expected = [Event::Level(2), Event::Skill(Skill::Hitpoints, 9), Event::Skill(Skill::Attack, 1)];
        assert_eq!(events, expected);
    }
}
