use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MIGRATIONS: &[&str] = &["
    CREATE TABLE accounts (
        id BLOB PRIMARY KEY,
        public_key BLOB NOT NULL UNIQUE,
        last_sequence INTEGER NOT NULL DEFAULT 0,
        created INTEGER NOT NULL,
        last_session INTEGER NOT NULL,
        last_news_seen INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE worlds (
        account_id BLOB PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
        revision INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        world TEXT NOT NULL
    );
    CREATE TABLE replies (
        account_id BLOB NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
        sequence INTEGER NOT NULL,
        tag BLOB NOT NULL,
        reply BLOB NOT NULL,
        PRIMARY KEY (account_id, sequence)
    );
    CREATE TABLE news (
        id INTEGER PRIMARY KEY,
        account_id BLOB NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
        created INTEGER NOT NULL,
        event TEXT NOT NULL
    );
    CREATE INDEX news_by_account ON news (account_id, id);
"];

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Store> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        migrate(&connection)?;
        Ok(Store { connection })
    }

    pub fn accounts(&self) -> rusqlite::Result<u32> {
        self.connection.query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
    }
}

fn migrate(connection: &Connection) -> rusqlite::Result<()> {
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version(connection)?) {
        connection.execute_batch(&format!("BEGIN; {migration} PRAGMA user_version = {}; COMMIT;", index + 1))?;
    }
    Ok(())
}

fn version(connection: &Connection) -> rusqlite::Result<usize> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(version as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrating_again_changes_nothing() {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        migrate(&connection).unwrap();
        assert_eq!(version(&connection).unwrap(), MIGRATIONS.len());
    }
}
