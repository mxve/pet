/*!
account:
  load and save ~/.pet/account.toml
*/

use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Serialize, Deserialize)]
pub struct Account {
    pub server: String,
    pub id: String,
    pub key: String,
}

fn path() -> Result<PathBuf> {
    let home = std::env::home_dir().ok_or("cannot find the home directory")?;
    Ok(home.join(".pet").join("account.toml"))
}

pub fn load() -> Result<Option<Account>> {
    let path = path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(at(&path, error).into()),
    };
    let account = toml::from_str(&text).map_err(|error| at(&path, error))?;
    Ok(Some(account))
}

/// owner only on unix, the key is the login. lost key = lost pet
pub fn store(account: &Account) -> Result<()> {
    let path = path()?;
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path).map_err(|error| at(&path, error))?;
    file.write_all(toml::to_string(account)?.as_bytes())?;
    Ok(())
}

fn at(path: &Path, problem: impl Display) -> String {
    format!("{}: {problem}", path.display())
}
