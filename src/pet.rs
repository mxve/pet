use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const FULL: f32 = 100.0;
pub const LOW: f32 = 25.0;
const HIGH: f32 = 70.0;
const CATCH_UP_STEP: Duration = Duration::from_secs(60);
const LEVEL_BASE_XP: f64 = 20.0;
const LEVELS_PER_DOUBLING: f64 = 7.0;
const MAX_LEVEL: u32 = 99;
const ASLEEP_XP_PER_HOUR: f32 = 1.0;
const AWAKE_RATE_PER_HOUR: Stats = Stats {
    food: -8.0,
    joy: -6.0,
    energy: -5.0,
};
const ASLEEP_RATE_PER_HOUR: Stats = Stats {
    food: -3.0,
    joy: 0.0,
    energy: 25.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub food: f32,
    pub joy: f32,
    pub energy: f32,
}

impl Stats {
    pub fn shifted(self, by: Stats) -> Stats {
        Stats {
            food: (self.food + by.food).clamp(0.0, FULL),
            joy: (self.joy + by.joy).clamp(0.0, FULL),
            energy: (self.energy + by.energy).clamp(0.0, FULL),
        }
    }

    fn scaled(self, factor: f32) -> Stats {
        Stats {
            food: self.food * factor,
            joy: self.joy * factor,
            energy: self.energy * factor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Asleep,
    Hungry,
    Bored,
    Tired,
    Content,
    Happy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Skill {
    Hitpoints,
    Attack,
    Defence,
    Speed,
    Stamina,
}

impl Skill {
    pub const ALL: [Skill; 5] = [
        Skill::Hitpoints,
        Skill::Attack,
        Skill::Defence,
        Skill::Speed,
        Skill::Stamina,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Skill::Hitpoints => "Hitpoints",
            Skill::Attack => "Attack",
            Skill::Defence => "Defence",
            Skill::Speed => "Speed",
            Skill::Stamina => "Stamina",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub number: u32,
    pub into: u32,
    pub needed: u32,
}

impl Level {
    pub fn ratio(self) -> f32 {
        if self.needed == 0 {
            1.0
        } else {
            self.into as f32 / self.needed as f32
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Pet {
    pub name: String,
    pub species: String,
    pub stats: Stats,
    pub asleep: bool,
    pub skills: BTreeMap<Skill, f32>,
    pub last_seen: u64,
}

impl Pet {
    pub fn new(name: &str, species: &str) -> Pet {
        Pet {
            name: name.to_string(),
            species: species.to_string(),
            stats: Stats {
                food: FULL,
                joy: FULL,
                energy: FULL,
            },
            asleep: false,
            skills: BTreeMap::from(Skill::ALL.map(|skill| (skill, 0.0))),
            last_seen: 0,
        }
    }

    pub fn apply(&mut self, effect: Stats) -> f32 {
        let before = self.stats;
        self.stats = self.stats.shifted(effect);
        self.asleep = false;
        let gain = |now: f32, then: f32| (now - then).max(0.0);
        gain(self.stats.food, before.food)
            + gain(self.stats.joy, before.joy)
            + gain(self.stats.energy, before.energy)
    }

    pub fn tick(&mut self, elapsed: Duration) {
        let hours = elapsed.as_secs_f32() / 3600.0;
        let rate = if self.asleep {
            ASLEEP_RATE_PER_HOUR
        } else {
            AWAKE_RATE_PER_HOUR
        };
        self.stats = self.stats.shifted(rate.scaled(hours));
        if self.asleep {
            self.earn(ASLEEP_XP_PER_HOUR * hours);
        }
        if self.stats.energy >= FULL {
            self.asleep = false;
        }
    }

    pub fn earn(&mut self, xp: f32) {
        *self.skills.entry(Skill::Hitpoints).or_default() += xp;
    }

    pub fn xp(&self, skill: Skill) -> f32 {
        self.skills.get(&skill).copied().unwrap_or(0.0)
    }

    pub fn skill(&self, skill: Skill) -> Level {
        level_at(self.xp(skill))
    }

    pub fn level(&self) -> u32 {
        Skill::ALL
            .iter()
            .map(|&skill| self.skill(skill).number)
            .sum()
    }

    pub fn advance(&mut self, elapsed: Duration) {
        let mut left = elapsed;
        while !left.is_zero() {
            let step = left.min(CATCH_UP_STEP);
            self.tick(step);
            left -= step;
        }
    }

    pub fn mood(&self) -> Mood {
        let Stats { food, joy, energy } = self.stats;
        let lowest = food.min(joy).min(energy);
        if self.asleep {
            Mood::Asleep
        } else if lowest >= HIGH {
            Mood::Happy
        } else if lowest >= LOW {
            Mood::Content
        } else if lowest == food {
            Mood::Hungry
        } else if lowest == joy {
            Mood::Bored
        } else {
            Mood::Tired
        }
    }
}

fn level_at(xp: f32) -> Level {
    let mut number = 1;
    let mut into = xp as u32;
    while number < MAX_LEVEL && into >= level_cost(number) {
        into -= level_cost(number);
        number += 1;
    }
    if number == MAX_LEVEL {
        return Level {
            number,
            into: 0,
            needed: 0,
        };
    }
    Level {
        number,
        into,
        needed: level_cost(number),
    }
}

fn level_cost(level: u32) -> u32 {
    let doublings = f64::from(level - 1) / LEVELS_PER_DOUBLING;
    (LEVEL_BASE_XP * doublings.exp2()).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_stops_at_zero() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.tick(Duration::from_secs(3600));
        assert_eq!(pet.stats.food, 92.0);
        pet.tick(Duration::from_secs(3600 * 1000));
        assert_eq!(
            pet.stats,
            Stats {
                food: 0.0,
                joy: 0.0,
                energy: 0.0
            }
        );
    }

    #[test]
    fn feeding_a_full_pet_stays_full() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.apply(Stats {
            food: 30.0,
            joy: 0.0,
            energy: 0.0,
        });
        assert_eq!(pet.stats.food, FULL);
    }

    #[test]
    fn mood_follows_the_lowest_stat() {
        let mut pet = Pet::new("Mochi", "Cat");
        assert_eq!(pet.mood(), Mood::Happy);
        pet.stats.joy = 50.0;
        assert_eq!(pet.mood(), Mood::Content);
        pet.stats.energy = 20.0;
        pet.stats.food = 10.0;
        assert_eq!(pet.mood(), Mood::Hungry);
        pet.stats.food = 30.0;
        assert_eq!(pet.mood(), Mood::Tired);
        pet.stats.joy = 20.0;
        pet.stats.energy = 20.0;
        assert_eq!(pet.mood(), Mood::Bored);
    }

    #[test]
    fn sleeping_restores_energy_then_wakes() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.stats.energy = 10.0;
        pet.asleep = true;
        pet.tick(Duration::from_secs(3600));
        assert_eq!(pet.stats.energy, 35.0);
        assert_eq!(pet.mood(), Mood::Asleep);
        pet.tick(Duration::from_secs(3600 * 3));
        assert_eq!(pet.stats.energy, FULL);
        assert!(!pet.asleep);
    }

    #[test]
    fn levels_cost_more_and_more_up_to_99() {
        let number = |xp| level_at(xp).number;
        assert_eq!(
            [number(19.0), number(20.0), number(92.0), number(93.0)],
            [1, 2, 4, 5]
        );
        assert_eq!(level_cost(8), 2 * level_cost(1));
        assert_eq!(number(1e9), MAX_LEVEL);
    }

    #[test]
    fn the_level_is_every_skill_added_up() {
        let mut pet = Pet::new("Mochi", "Cat");
        assert_eq!(pet.level(), 5);
        pet.earn(20.0);
        assert_eq!(pet.level(), 6);
    }

    #[test]
    fn sleeping_earns_a_little_xp() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.stats.energy = 0.0;
        pet.asleep = true;
        pet.advance(Duration::from_secs(150 * 60));
        assert!((pet.xp(Skill::Hitpoints) - 2.5).abs() < 0.01);
        let mut awake = Pet::new("Mochi", "Cat");
        awake.advance(Duration::from_secs(150 * 60));
        assert_eq!(awake.xp(Skill::Hitpoints), 0.0);
    }

    #[test]
    fn catching_up_matches_ticking_minute_by_minute() {
        let tired = || {
            let mut pet = Pet::new("Mochi", "Cat");
            pet.stats.energy = 10.0;
            pet.asleep = true;
            pet
        };
        let mut caught_up = tired();
        caught_up.advance(Duration::from_secs(10 * 3600));
        let mut ticked = tired();
        for _ in 0..600 {
            ticked.tick(Duration::from_secs(60));
        }
        assert_eq!(caught_up, ticked);
        assert!(!caught_up.asleep);
    }
}
