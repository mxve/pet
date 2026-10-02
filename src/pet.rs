use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const FULL: f32 = 100.0;
pub const LOW: f32 = 25.0;
const HIGH: f32 = 70.0;
const CATCH_UP_STEP: Duration = Duration::from_secs(60);
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
    fn shifted(self, by: Stats) -> Stats {
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Pet {
    pub stats: Stats,
    pub asleep: bool,
    pub last_seen: u64,
}

impl Pet {
    pub fn new() -> Pet {
        Pet {
            stats: Stats {
                food: FULL,
                joy: FULL,
                energy: FULL,
            },
            asleep: false,
            last_seen: 0,
        }
    }

    pub fn apply(&mut self, effect: Stats) {
        self.stats = self.stats.shifted(effect);
        self.asleep = false;
    }

    pub fn tick(&mut self, elapsed: Duration) {
        let hours = elapsed.as_secs_f32() / 3600.0;
        let rate = if self.asleep {
            ASLEEP_RATE_PER_HOUR
        } else {
            AWAKE_RATE_PER_HOUR
        };
        self.stats = self.stats.shifted(rate.scaled(hours));
        if self.stats.energy >= FULL {
            self.asleep = false;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_stops_at_zero() {
        let mut pet = Pet::new();
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
        let mut pet = Pet::new();
        pet.apply(Stats {
            food: 30.0,
            joy: 0.0,
            energy: 0.0,
        });
        assert_eq!(pet.stats.food, FULL);
    }

    #[test]
    fn mood_follows_the_lowest_stat() {
        let mut pet = Pet::new();
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
        let mut pet = Pet::new();
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
    fn catching_up_matches_ticking_minute_by_minute() {
        let tired = || {
            let mut pet = Pet::new();
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
