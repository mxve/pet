/*!
pet:
  stats and moods
  skills and the xp curve
  names
*/

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const FULL: f32 = 100.0;
pub const LOW: f32 = 25.0;
/// the server cant read pet files, so it keeps its own list
pub const SPECIES: [&str; 5] = ["Bunny", "Cat", "Dog", "Fox", "Slime"];
pub const NAME_LENGTH: usize = 12;
const HIGH: f32 = 70.0;
/// small steps, so being away plays out like being watched
const CATCH_UP_STEP: Duration = Duration::from_secs(60);
const LEVEL_BASE_XP: f64 = 10.0;
const LEVEL_GROWTH: f64 = 1.08;
const MAX_LEVEL: u32 = 99;
/// float error would leave you a level short right at the boundary
const BOUNDARY_SLACK: f64 = 1e-9;
/// training everything at once is slow on purpose
const ALL_SKILLS_SHARE: f32 = 0.1;
const HAPPY_XP_PER_HOUR: f32 = 8.0;
const ASLEEP_XP_PER_HOUR: f32 = 4.0;
const TRAINING_XP_PER_HOUR: f32 = 120.0;
const STAMINA_SAVING_PER_LEVEL: f32 = 0.01;
/// stamina helps, energy never gets free
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
    pub const fn new(food: f32, joy: f32, energy: f32) -> Stats {
        Stats { food, joy, energy }
    }

    pub(crate) fn shifted(self, by: Stats) -> Stats {
        self.zip(by, |stat, change| (stat + change).clamp(0.0, FULL))
    }

    fn scaled(self, factor: f32) -> Stats {
        self.zip(Stats::new(factor, factor, factor), |stat, factor| stat * factor)
    }

    fn sum(self) -> f32 {
        self.food + self.joy + self.energy
    }

    fn zip(self, other: Stats, combine: impl Fn(f32, f32) -> f32) -> Stats {
        Stats::new(
            combine(self.food, other.food),
            combine(self.joy, other.joy),
            combine(self.energy, other.energy),
        )
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
    /// max level needs nothing, so the bar shows full
    pub fn ratio(self) -> f32 {
        if self.needed == 0 {
            1.0
        } else {
            self.into as f32 / self.needed as f32
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pet {
    pub name: String,
    pub species: String,
    pub stats: Stats,
    pub activity: Activity,
    pub skills: BTreeMap<Skill, f32>,
    pub focus: Focus,
}

impl Pet {
    pub fn new(name: &str, species: &str) -> Pet {
        Pet {
            name: name.to_string(),
            species: species.to_string(),
            stats: Stats::new(FULL, FULL, FULL),
            activity: Activity::Awake,
            skills: BTreeMap::from(Skill::ALL.map(|skill| (skill, 0.0))),
            focus: Focus::All,
        }
    }

    /// any care wakes the pet. returns only what helped, so feeding a full pet earns nothing
    pub(crate) fn apply(&mut self, effect: Stats) -> f32 {
        let before = self.stats;
        self.stats = self.stats.shifted(effect);
        if self.activity == Activity::Asleep {
            self.activity = Activity::Awake;
        }
        self.stats.zip(before, |now, then| (now - then).max(0.0)).sum()
    }

    /// stamina only slows energy loss, sleep refills at full speed
    pub(crate) fn tick(&mut self, elapsed: Duration) {
        let hours = elapsed.as_secs_f32() / 3600.0;
        let (mut rate, xp_per_hour) = match self.activity {
            Activity::Awake if self.mood() == Mood::Happy => (AWAKE_RATE_PER_HOUR, HAPPY_XP_PER_HOUR),
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

    pub(crate) fn earn(&mut self, xp: f32) {
        match self.focus {
            Focus::One(skill) => *self.skills.entry(skill).or_default() += xp,
            Focus::All => {
                for skill in Skill::ALL {
                    *self.skills.entry(skill).or_default() += xp * ALL_SKILLS_SHARE;
                }
            }
        }
    }

    /// with all skills on, show the one closest to leveling
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

    pub fn total_xp(&self) -> f32 {
        Skill::ALL.iter().map(|&skill| self.xp(skill)).sum()
    }

    pub fn level(&self) -> Level {
        level_at(self.total_xp(), Skill::ALL.len() as u32)
    }

    pub fn total_level(&self) -> u32 {
        Skill::ALL.iter().map(|&skill| self.skill(skill).number).sum()
    }

    /// same result as ticking live, no matter how long you were gone
    pub(crate) fn advance(&mut self, elapsed: Duration) {
        let mut left = elapsed;
        while !left.is_zero() {
            let step = left.min(CATCH_UP_STEP);
            self.tick(step);
            left -= step;
        }
    }

    /// the worst stat sets the mood, food wins ties
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

/// the pet level is the same curve, `scale` times the xp per level
fn level_at(xp: f32, scale: u32) -> Level {
    let scale = f64::from(scale);
    let xp = f64::from(xp.max(0.0)) / scale;
    let reached = (1.0 + xp * (LEVEL_GROWTH - 1.0) / LEVEL_BASE_XP).ln() / LEVEL_GROWTH.ln() + BOUNDARY_SLACK;
    let number = (reached.floor() as u32).min(MAX_LEVEL);
    if number == MAX_LEVEL {
        return Level {
            number,
            into: 0,
            needed: 0,
        };
    }
    Level {
        number,
        into: ((xp - total_xp(number)).max(0.0) * scale) as u32,
        needed: (level_cost(number) * scale).ceil() as u32,
    }
}

fn total_xp(level: u32) -> f64 {
    LEVEL_BASE_XP * (LEVEL_GROWTH.powi(level as i32) - 1.0) / (LEVEL_GROWTH - 1.0)
}

fn level_cost(level: u32) -> f64 {
    LEVEL_BASE_XP * LEVEL_GROWTH.powi(level as i32)
}

/// in core so client and server agree on names
pub fn valid_name(name: &str) -> bool {
    let length = name.chars().count();
    name == name.trim() && (1..=NAME_LENGTH).contains(&length) && !name.chars().any(hides_text)
}

/// no control or bidi chars, names cant hide text
fn hides_text(character: char) -> bool {
    character.is_control() || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!([number(9.9), number(10.0), number(58.0), number(59.0)], [0, 1, 4, 5]);
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
        pet.focus = Focus::One(Skill::Hitpoints);
        pet.activity = Activity::Training;
        pet.tick(Duration::from_secs(3600));
        assert_eq!(pet.xp(Skill::Hitpoints), 120.0);
        assert_eq!((pet.stats.food, pet.stats.energy), (80.0, 60.0));
        pet.advance(Duration::from_secs(10 * 3600));
        assert_eq!(pet.activity, Activity::Awake);
        assert_eq!(pet.stats.energy, 0.0);
        assert!(pet.xp(Skill::Hitpoints) < 320.0);
    }

    #[test]
    fn stamina_slows_energy_loss() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.skills.insert(Skill::Stamina, total_xp(20).ceil() as f32);
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
        pet.focus = Focus::One(Skill::Hitpoints);
        pet.earn(25.0);
        assert_eq!(pet.level().number, 0);
        assert_eq!(pet.level().ratio(), 0.5);
        assert_eq!(pet.total_level(), 2);
        pet.earn(25.0);
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
