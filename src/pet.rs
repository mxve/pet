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
const ALL_SKILLS_SHARE: f32 = 0.1;
const ASLEEP_XP_PER_HOUR: f32 = 1.0;
const TRAINING_XP_PER_HOUR: f32 = 30.0;
const STAMINA_SAVING_PER_LEVEL: f32 = 0.01;
const MAX_STAMINA_SAVING: f32 = 0.5;
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
const TRAINING_RATE_PER_HOUR: Stats = Stats {
    food: -20.0,
    joy: -6.0,
    energy: -40.0,
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
    Training,
    Hungry,
    Bored,
    Tired,
    Content,
    Happy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    Awake,
    Asleep,
    Training,
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
    pub const ALL: [Skill; 5] = [Skill::Hitpoints, Skill::Attack, Skill::Defence, Skill::Speed, Skill::Stamina];

    pub fn name(self) -> &'static str {
        match self {
            Skill::Hitpoints => "Hitpoints",
            Skill::Attack => "Attack",
            Skill::Defence => "Defence",
            Skill::Speed => "Speed",
            Skill::Stamina => "Stamina",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Skill::Hitpoints => "HIT",
            Skill::Attack => "ATK",
            Skill::Defence => "DEF",
            Skill::Speed => "SPD",
            Skill::Stamina => "STA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Focus {
    One(Skill),
    All,
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
    pub activity: Activity,
    pub skills: BTreeMap<Skill, f32>,
    pub focus: Focus,
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
            activity: Activity::Awake,
            skills: BTreeMap::from(Skill::ALL.map(|skill| (skill, 0.0))),
            focus: Focus::One(Skill::Hitpoints),
            last_seen: 0,
        }
    }

    pub fn apply(&mut self, effect: Stats) -> f32 {
        let before = self.stats;
        self.stats = self.stats.shifted(effect);
        if self.activity == Activity::Asleep {
            self.activity = Activity::Awake;
        }
        let gain = |now: f32, then: f32| (now - then).max(0.0);
        gain(self.stats.food, before.food) + gain(self.stats.joy, before.joy) + gain(self.stats.energy, before.energy)
    }

    pub fn tick(&mut self, elapsed: Duration) {
        let hours = elapsed.as_secs_f32() / 3600.0;
        let (mut rate, xp_per_hour) = match self.activity {
            Activity::Awake => (AWAKE_RATE_PER_HOUR, 0.0),
            Activity::Asleep => (ASLEEP_RATE_PER_HOUR, ASLEEP_XP_PER_HOUR),
            Activity::Training => (TRAINING_RATE_PER_HOUR, TRAINING_XP_PER_HOUR),
        };
        if rate.energy < 0.0 {
            rate.energy *= 1.0 - self.stamina_saving();
        }
        self.stats = self.stats.shifted(rate.scaled(hours));
        self.earn(xp_per_hour * hours);
        let rested = self.stats.energy >= FULL;
        let spent = self.stats.food <= 0.0 || self.stats.energy <= 0.0;
        match self.activity {
            Activity::Asleep if rested => self.activity = Activity::Awake,
            Activity::Training if spent => self.activity = Activity::Awake,
            Activity::Awake | Activity::Asleep | Activity::Training => {}
        }
    }

    fn stamina_saving(&self) -> f32 {
        let levels = self.skill(Skill::Stamina).number;
        (levels as f32 * STAMINA_SAVING_PER_LEVEL).min(MAX_STAMINA_SAVING)
    }

    pub fn earn(&mut self, xp: f32) {
        match self.focus {
            Focus::One(skill) => *self.skills.entry(skill).or_default() += xp,
            Focus::All => {
                for skill in Skill::ALL {
                    *self.skills.entry(skill).or_default() += xp * ALL_SKILLS_SHARE;
                }
            }
        }
    }

    pub fn tracked(&self) -> Skill {
        match self.focus {
            Focus::One(skill) => skill,
            Focus::All => Skill::ALL
                .into_iter()
                .max_by(|a, b| self.skill(*a).ratio().total_cmp(&self.skill(*b).ratio()))
                .unwrap_or(Skill::Hitpoints),
        }
    }

    pub fn xp(&self, skill: Skill) -> f32 {
        self.skills.get(&skill).copied().unwrap_or(0.0)
    }

    pub fn skill(&self, skill: Skill) -> Level {
        level_at(self.xp(skill), 1)
    }

    pub fn level(&self) -> Level {
        let total: f32 = Skill::ALL.iter().map(|&skill| self.xp(skill)).sum();
        level_at(total, Skill::ALL.len() as u32)
    }

    pub fn total_level(&self) -> u32 {
        Skill::ALL.iter().map(|&skill| self.skill(skill).number).sum()
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
        match self.activity {
            Activity::Asleep => return Mood::Asleep,
            Activity::Training => return Mood::Training,
            Activity::Awake => {}
        }
        if lowest >= HIGH {
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

fn level_at(xp: f32, scale: u32) -> Level {
    let cost = |number| level_cost(number) * scale;
    let mut number = 0;
    let mut into = xp as u32;
    while number < MAX_LEVEL && into >= cost(number) {
        into -= cost(number);
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
        needed: cost(number),
    }
}

fn level_cost(level: u32) -> u32 {
    let doublings = f64::from(level) / LEVELS_PER_DOUBLING;
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
        pet.activity = Activity::Asleep;
        pet.tick(Duration::from_secs(3600));
        assert_eq!(pet.stats.energy, 35.0);
        assert_eq!(pet.mood(), Mood::Asleep);
        pet.tick(Duration::from_secs(3600 * 3));
        assert_eq!(pet.stats.energy, FULL);
        assert_eq!(pet.activity, Activity::Awake);
    }

    #[test]
    fn levels_cost_more_and_more_up_to_99() {
        let number = |xp| level_at(xp, 1).number;
        assert_eq!([number(19.0), number(20.0), number(92.0), number(93.0)], [0, 1, 3, 4]);
        assert_eq!(level_cost(7), 2 * level_cost(0));
        assert_eq!(number(1e9), MAX_LEVEL);
    }

    #[test]
    fn xp_follows_the_focus() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.focus = Focus::One(Skill::Attack);
        pet.earn(10.0);
        assert_eq!(pet.xp(Skill::Attack), 10.0);
        assert_eq!(pet.xp(Skill::Hitpoints), 0.0);
        pet.focus = Focus::All;
        pet.earn(10.0);
        assert_eq!(pet.xp(Skill::Attack), 11.0);
        assert_eq!(pet.xp(Skill::Stamina), 1.0);
    }

    #[test]
    fn training_earns_xp_until_food_or_energy_runs_out() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.activity = Activity::Training;
        pet.tick(Duration::from_secs(3600));
        assert_eq!(pet.xp(Skill::Hitpoints), 30.0);
        assert_eq!((pet.stats.food, pet.stats.energy), (80.0, 60.0));
        pet.advance(Duration::from_secs(10 * 3600));
        assert_eq!(pet.activity, Activity::Awake);
        assert_eq!(pet.stats.energy, 0.0);
        assert!(pet.xp(Skill::Hitpoints) < 80.0);
    }

    #[test]
    fn stamina_slows_energy_loss() {
        let mut pet = Pet::new("Mochi", "Cat");
        let to_level_20: u32 = (0..20).map(level_cost).sum();
        pet.skills.insert(Skill::Stamina, to_level_20 as f32);
        assert_eq!(pet.skill(Skill::Stamina).number, 20);
        pet.tick(Duration::from_secs(3600));
        assert!((pet.stats.energy - 96.0).abs() < 0.001);
        pet.stats.energy = 50.0;
        pet.activity = Activity::Asleep;
        pet.tick(Duration::from_secs(3600));
        assert_eq!(pet.stats.energy, 75.0);
    }

    #[test]
    fn a_pet_level_costs_one_level_of_every_skill() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.earn(50.0);
        assert_eq!(pet.level().number, 0);
        assert_eq!(pet.level().ratio(), 0.5);
        assert_eq!(pet.total_level(), 2);
        pet.earn(50.0);
        assert_eq!(pet.level().number, 1);
    }

    #[test]
    fn catching_up_matches_ticking_minute_by_minute() {
        let tired = || {
            let mut pet = Pet::new("Mochi", "Cat");
            pet.stats.energy = 10.0;
            pet.activity = Activity::Asleep;
            pet
        };
        let mut caught_up = tired();
        caught_up.advance(Duration::from_secs(10 * 3600));
        let mut ticked = tired();
        for _ in 0..600 {
            ticked.tick(Duration::from_secs(60));
        }
        assert_eq!(caught_up, ticked);
        assert_eq!(caught_up.activity, Activity::Awake);
    }
}
