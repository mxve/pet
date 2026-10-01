use std::time::Duration;

pub const FULL: f32 = 100.0;
const AWAKE_RATE_PER_HOUR: Stats = Stats {
    food: -8.0,
    joy: -6.0,
    energy: -5.0,
};

#[derive(Debug, Clone, Copy, PartialEq)]
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

pub struct Pet {
    pub stats: Stats,
}

impl Pet {
    pub fn new() -> Pet {
        Pet {
            stats: Stats {
                food: FULL,
                joy: FULL,
                energy: FULL,
            },
        }
    }

    pub fn apply(&mut self, effect: Stats) {
        self.stats = self.stats.shifted(effect);
    }

    pub fn tick(&mut self, elapsed: Duration) {
        let hours = elapsed.as_secs_f32() / 3600.0;
        self.stats = self.stats.shifted(AWAKE_RATE_PER_HOUR.scaled(hours));
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
}
