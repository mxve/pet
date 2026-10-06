use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Result;
use pet_core::pet::Pet;

pub fn path() -> Result<PathBuf> {
    let home = std::env::home_dir().ok_or("cannot find the home directory")?;
    Ok(home.join(".pet").join("save.toml"))
}

pub fn load() -> Result<Option<Pet>> {
    let path = path()?;
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display()).into()),
    };
    let pet = toml::from_str(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Some(pet))
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

pub fn store(pet: &mut Pet) -> Result<()> {
    pet.last_seen = now();
    let path = path()?;
    let temporary = path.with_extension("toml.tmp");
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(&temporary, toml::to_string(pet)?)?;
    fs::rename(&temporary, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pet_core::pet::{Activity, Pet};

    #[test]
    fn a_pet_survives_the_save_format() {
        let mut pet = Pet::new("Mochi", "Cat");
        pet.stats.food = 12.5;
        pet.activity = Activity::Asleep;
        let saved = toml::to_string(&pet).unwrap();
        assert_eq!(toml::from_str::<Pet>(&saved).unwrap(), pet);
    }
}
