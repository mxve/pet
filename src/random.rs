use std::hash::{DefaultHasher, Hash, Hasher};

pub fn roll(key: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}
