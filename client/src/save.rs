use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::Result;
use pet_core::world::World;

pub fn path() -> Result<PathBuf> {
    let home = std::env::home_dir().ok_or("cannot find the home directory")?;
    Ok(home.join(".pet").join("save.toml"))
}

pub fn load() -> Result<Option<World>> {
    let path = path()?;
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display()).into()),
    };
    let world = toml::from_str(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Some(world))
}

pub fn now() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
}

pub fn store(world: &World) -> Result<()> {
    let path = path()?;
    let temporary = path.with_extension("toml.tmp");
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(&temporary, toml::to_string(world)?)?;
    fs::rename(&temporary, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pet_core::pet::Pet;
    use pet_core::world::World;

    #[test]
    fn a_world_survives_the_save_format() {
        let world = World::new(Pet::new("Mochi", "Cat"), Duration::from_secs(1_759_000_000));
        let saved = toml::to_string(&world).unwrap();
        assert_eq!(toml::from_str::<World>(&saved).unwrap(), world);
    }
}
