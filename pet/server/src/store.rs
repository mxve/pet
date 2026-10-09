/*!
store:
  sqlite
  accounts and worlds
  stored replies
  migrations
*/

use std::path::Path;
use std::time::Duration;

use pet_core::protocol::{AccountId, Key};
use pet_core::world::World;
use rusqlite::{Connection, OptionalExtension, params};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const KEPT_REPLIES: i64 = 16;
/// append only, never edit one that shipped
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

/// stored so a retry gets the exact same bytes back
pub struct Answer<'a> {
    pub tag: &'a [u8],
    pub reply: &'a [u8],
}

pub struct Store {
    connection: Connection,
}

impl Store {
    /// wal + normal: fast; power loss will result in missing last writes but uncorrupted db
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

    pub fn register(&self, account: &AccountId, public: &Key, world: &World) -> Result<()> {
        let created = world.last_seen().as_secs() as i64;
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO accounts (id, public_key, created, last_session) VALUES (?1, ?2, ?3, ?3)",
            params![&account[..], &public[..], created],
        )?;
        transaction.execute(
            "INSERT INTO worlds (account_id, revision, last_seen, world) VALUES (?1, 0, ?2, ?3)",
            params![&account[..], created, toml::to_string(world)?],
        )?;
        Ok(transaction.commit()?)
    }

    pub fn public_key(&self, account: &AccountId) -> rusqlite::Result<Option<Key>> {
        self.connection
            .query_row("SELECT public_key FROM accounts WHERE id = ?1", [&account[..]], |row| row.get(0))
            .optional()
    }

    pub fn world(&self, account: &AccountId) -> Result<World> {
        let text: String = self
            .connection
            .query_row("SELECT world FROM worlds WHERE account_id = ?1", [&account[..]], |row| row.get(0))?;
        Ok(toml::from_str(&text)?)
    }

    pub fn reply(&self, account: &AccountId, sequence: i64, tag: &[u8]) -> rusqlite::Result<Option<Vec<u8>>> {
        self.connection
            .query_row(
                "SELECT reply FROM replies WHERE account_id = ?1 AND sequence = ?2 AND tag = ?3",
                params![&account[..], sequence, tag],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn last_sequence(&self, account: &AccountId) -> rusqlite::Result<i64> {
        self.connection
            .query_row("SELECT last_sequence FROM accounts WHERE id = ?1", [&account[..]], |row| row.get(0))
    }

    pub fn save_command(&self, account: &AccountId, sequence: i64, world: &World, answer: Answer) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO replies (account_id, sequence, tag, reply) VALUES (?1, ?2, ?3, ?4)",
            params![&account[..], sequence, answer.tag, answer.reply],
        )?;
        transaction.execute(
            "DELETE FROM replies WHERE account_id = ?1 AND sequence NOT IN
                (SELECT sequence FROM replies WHERE account_id = ?1 ORDER BY sequence DESC LIMIT ?2)",
            params![&account[..], KEPT_REPLIES],
        )?;
        transaction.execute(
            "UPDATE accounts SET last_sequence = ?2 WHERE id = ?1",
            params![&account[..], sequence],
        )?;
        write_world(&transaction, account, world)?;
        Ok(transaction.commit()?)
    }

    pub fn save_world(&self, account: &AccountId, world: &World) -> Result<()> {
        write_world(&self.connection, account, world)
    }
}

fn write_world(connection: &Connection, account: &AccountId, world: &World) -> Result<()> {
    connection.execute(
        "UPDATE worlds SET revision = ?2, last_seen = ?3, world = ?4 WHERE account_id = ?1",
        params![
            &account[..],
            i64::try_from(world.revision())?,
            world.last_seen().as_secs() as i64,
            toml::to_string(world)?
        ],
    )?;
    Ok(())
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
