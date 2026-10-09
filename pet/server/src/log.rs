/*!
log:
  to stderr and a file
  levels
  utc timestamps
*/

use std::fmt::{self, Arguments};
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const GRAY: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";

static LOGGER: OnceLock<Logger> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn parse(text: &str) -> Option<Level> {
        match text.to_ascii_lowercase().as_str() {
            "debug" => Some(Level::Debug),
            "info" => Some(Level::Info),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
            Level::Error => "ERROR",
        }
    }

    fn color(self) -> &'static str {
        match self {
            Level::Debug => "\x1b[36m",
            Level::Info => "\x1b[32m",
            Level::Warn => "\x1b[33m",
            Level::Error => "\x1b[31m",
        }
    }
}

struct Logger {
    level: Level,
    colored: bool,
    file: Mutex<File>,
}

pub fn start(path: &Path, level: Level) -> io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let logger = Logger {
        level,
        colored: io::stderr().is_terminal(),
        file: Mutex::new(file),
    };
    LOGGER.set(logger).ok();
    Ok(())
}

pub fn write(level: Level, message: Arguments) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    if level < logger.level {
        return;
    }

    let time = Timestamp::now();
    if logger.colored {
        eprintln!("{GRAY}{time}{RESET} {}{}{RESET} {message}", level.color(), level.name());
    } else {
        eprintln!("{time} {} {message}", level.name());
    }
    if let Ok(mut file) = logger.file.lock() {
        writeln!(file, "{time} {} {message}", level.name()).ok();
    }
}

struct Timestamp {
    seconds: i64,
    millis: u32,
}

impl Timestamp {
    fn now() -> Timestamp {
        let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Timestamp {
            seconds: since.as_secs() as i64,
            millis: since.subsec_millis(),
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let (year, month, day) = civil_date(self.seconds.div_euclid(86_400));
        let second = self.seconds.rem_euclid(86_400);
        let (hour, minute, second) = (second / 3600, second / 60 % 60, second % 60);
        write!(
            formatter,
            "{year}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{:03}Z",
            self.millis
        )
    }
}

/// i totally wrote this myself and didnt steal it from
/// https://howardhinnant.github.io/date_algorithms.html#civil_from_days
fn civil_date(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

macro_rules! debug {
    ($($argument:tt)*) => { $crate::log::write($crate::log::Level::Debug, format_args!($($argument)*)) };
}

macro_rules! info {
    ($($argument:tt)*) => { $crate::log::write($crate::log::Level::Info, format_args!($($argument)*)) };
}

macro_rules! warning {
    ($($argument:tt)*) => { $crate::log::write($crate::log::Level::Warn, format_args!($($argument)*)) };
}

macro_rules! error {
    ($($argument:tt)*) => { $crate::log::write($crate::log::Level::Error, format_args!($($argument)*)) };
}

pub(crate) use {debug, error, info, warning};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_calendar_time() {
        let at = |seconds| Timestamp { seconds, millis: 7 }.to_string();
        assert_eq!(at(0), "1970-01-01 00:00:00.007Z");
        assert_eq!(at(951_782_400), "2000-02-29 00:00:00.007Z");
        assert_eq!(at(1_700_000_000), "2023-11-14 22:13:20.007Z");
    }
}
