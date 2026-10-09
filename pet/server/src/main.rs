/*!
pet server:
  flags and startup
  udp loop
  signups and logins
  commands and syncs
*/

mod log;
mod store;

use std::fmt::Display;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pet_core::pet::{self, Pet};
use pet_core::protocol::{
    self, AccountId, Challenge, Info, MAX_PACKET, PORT, Packet, PublicKey, Reply, Request, Secret, Signup, StaticSecret,
};
use pet_core::world::{Command, World};

use crate::log::{Level, debug, error, info, warning};
use crate::store::{Answer, Store};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DEFAULT_DB: &str = "pet.db";
const DEFAULT_NAME: &str = "pet";
const DEFAULT_LOG: &str = "pet-server.log";
/// prevent saving to db on every single sync (~4/sec)
const SAVE_CAUGHT_UP_AFTER: Duration = Duration::from_secs(60);

struct Server {
    name: String,
    key: StaticSecret,
    challenge_secret: Secret,
    store: Store,
}

fn main() -> Result<()> {
    let port = flag("--port").and_then(|port| port.parse().ok()).unwrap_or(PORT);
    let key = flag("--key")
        .or_else(|| std::env::var("PET_KEY_FILE").ok())
        .ok_or("pet-server needs --key <file> or PET_KEY_FILE, made by scripts/keygen.sh")?;
    let database = PathBuf::from(flag("--db").unwrap_or_else(|| DEFAULT_DB.to_string()));
    let log_file = PathBuf::from(flag("--log").unwrap_or_else(|| DEFAULT_LOG.to_string()));
    let level = match flag("--log-level").or_else(|| std::env::var("PET_LOG").ok()) {
        Some(level) => Level::parse(&level).ok_or_else(|| format!("unknown log level \"{level}\", use debug, info, warn or error"))?,
        None => Level::Info,
    };

    log::start(&log_file, level).map_err(|error| at(&log_file, error))?;
    let server = Server {
        name: flag("--name").unwrap_or_else(|| DEFAULT_NAME.to_string()),
        key: load_key(Path::new(&key))?,
        challenge_secret: protocol::random(),
        store: Store::open(&database).map_err(|error| at(&database, error))?,
    };
    let socket = UdpSocket::bind(("0.0.0.0", port)).inspect_err(|problem| {
        error!("server failed to start {{ port: {port}, error: {:?} }}", problem.to_string());
    })?;
    let public = protocol::to_hex(PublicKey::from(&server.key).as_bytes());
    let accounts = server.store.accounts().unwrap_or_default();
    let shown = format!("{level:?}").to_lowercase();

    info!("server started {{ port: {port}, database: {database:?}, accounts: {accounts} }}");
    info!("server key {{ public: {public} }}");
    info!("logging {{ level: {shown}, file: {log_file:?} }}");

    let mut buffer = [0; MAX_PACKET + 1];
    loop {
        let (size, from) = match socket.recv_from(&mut buffer) {
            Ok(received) => received,
            Err(problem) => {
                warning!("receive failed {{ error: {:?} }}", problem.to_string());
                continue;
            }
        };
        if let Some(reply) = server.answer(&buffer[..size], from)
            && let Err(problem) = socket.send_to(&reply, from)
        {
            warning!("send failed {{ to: {from}, error: {:?} }}", problem.to_string());
        }
    }
}

fn load_key(path: &Path) -> Result<StaticSecret> {
    let text = std::fs::read_to_string(path).map_err(|error| at(path, error))?;
    let bytes = protocol::from_hex(&text).ok_or_else(|| at(path, "not 64 hex characters"))?;
    Ok(StaticSecret::from(bytes))
}

fn at(path: &Path, problem: impl Display) -> String {
    format!("{}: {problem}", path.display())
}

impl Server {
    fn answer(&self, request: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
        let Some(packet) = protocol::decode(request) else {
            return self.signed(request, from);
        };
        match packet {
            Packet::GetInfo { .. } => {
                debug!("info asked {{ from: {from} }}");
                unsigned(self.info()?, request, from)
            }
            Packet::GetChallenge { nonce, .. } => {
                debug!("challenge asked {{ from: {from} }}");
                unsigned(self.challenge(nonce, from), request, from)
            }
            Packet::Register(signup) => self.register(signup, from),
            Packet::Info(_) | Packet::Challenge { .. } | Packet::Request { .. } | Packet::Reply { .. } => {
                debug!("packet dropped {{ from: {from}, reason: \"only the server sends it\" }}");
                None
            }
        }
    }

    fn info(&self) -> Option<Packet> {
        let info = Info {
            name: self.name.clone(),
            players: logged(self.store.accounts(), "counting accounts")?,
        };
        Some(Packet::Info(info))
    }

    fn challenge(&self, nonce: u64, from: SocketAddr) -> Packet {
        let challenge = protocol::challenge(&self.challenge_secret, from, minute());
        Packet::Challenge { nonce, challenge }
    }

    /// we trust the previous minute so time passed between send and parse doesn't invalidate the challenge
    fn challenge_fits(&self, challenge: &Challenge, from: SocketAddr) -> bool {
        let now = minute();
        [now, now.saturating_sub(1)]
            .into_iter()
            .any(|minute| protocol::challenge(&self.challenge_secret, from, minute) == *challenge)
    }

    fn register(&self, signup: Signup, from: SocketAddr) -> Option<Vec<u8>> {
        let (name, species) = (signup.name.as_str(), signup.species.as_str());
        if !self.challenge_fits(&signup.challenge, from) {
            debug!("signup dropped {{ from: {from}, reason: \"challenge does not fit\", name: {name:?}, species: {species:?} }}");
            return None;
        }
        let refusal = if !pet::valid_name(name) {
            Some("invalid name")
        } else if !pet::SPECIES.contains(&species) {
            Some("unknown species")
        } else {
            None
        };
        if let Some(refusal) = refusal {
            info!("signup refused {{ from: {from}, reason: {refusal:?}, name: {name:?}, species: {species:?} }}");
            return None;
        }

        let account = protocol::random();
        let world = World::new(Pet::new(name, species), now());
        logged(self.store.register(&account, &signup.public, &world), "storing a signup")?;
        let id = protocol::to_hex(&account);

        info!("signed up {{ from: {from}, account: {id}, name: {name:?}, species: {species:?} }}");

        let secret = protocol::account_secret(&self.key, &PublicKey::from(signup.public));
        let reply = Packet::Reply {
            sequence: 0,
            reply: Reply::Registered { account },
        };
        Some(protocol::seal(&reply, &secret))
    }

    fn signed(&self, bytes: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
        let Some(Packet::Request {
            account,
            challenge,
            sequence,
            request,
        }) = protocol::unverified(bytes)
        else {
            debug!(
                "packet dropped {{ from: {from}, reason: \"not a packet\", bytes: {} }}",
                bytes.len()
            );
            return None;
        };
        let id = protocol::to_hex(&account);
        if !self.challenge_fits(&challenge, from) {
            debug!("request dropped {{ from: {from}, account: {id}, reason: \"challenge does not fit\" }}");
            return None;
        }
        let Some(public) = logged(self.store.public_key(&account), "looking up an account")? else {
            debug!("login failed {{ from: {from}, account: {id}, reason: \"no such account\" }}");
            return None;
        };
        let secret = protocol::account_secret(&self.key, &PublicKey::from(public));
        if protocol::open(bytes, &secret).is_none() {
            debug!("login failed {{ from: {from}, account: {id}, reason: \"wrong signature\" }}");
            return None;
        }

        match request {
            Request::Sync { since } => {
                let reply = self.sync(&account, since, from)?;
                Some(protocol::seal(&Packet::Reply { sequence, reply }, &secret))
            }
            Request::Command { command } => self.command(&account, sequence, command, from, bytes, &secret),
        }
    }

    /// stored reply first, so a retry works even after the sequence moved on.
    /// cheats require server in dev mode, no privileged users
    fn command(
        &self,
        account: &AccountId,
        sequence: u64,
        command: Command,
        from: SocketAddr,
        request: &[u8],
        secret: &Secret,
    ) -> Option<Vec<u8>> {
        let id = protocol::to_hex(account);
        let order = i64::try_from(sequence).ok()?;
        let tag = protocol::tag(request)?;
        if let Some(reply) = logged(self.store.reply(account, order, tag), "loading a reply")? {
            debug!("command repeated {{ from: {from}, account: {id}, sequence: {sequence} }}");
            return Some(reply);
        }
        let last = logged(self.store.last_sequence(account), "loading a sequence")?;
        if order <= last {
            debug!("command dropped {{ from: {from}, account: {id}, reason: \"old sequence\", sequence: {sequence}, last: {last} }}");
            return None;
        }
        #[cfg(debug_assertions)]
        if let Command::Cheat(_) = command {
            debug!("command dropped {{ from: {from}, account: {id}, reason: \"cheats need a dev server\" }}");
            return None;
        }

        let mut world = logged(self.store.world(account), "loading a world")?;
        let now = now();
        world.catch_up(now);
        let reply = match world.apply(command, now) {
            Ok(_) => {
                let revision = world.revision();
                info!("command applied {{ from: {from}, account: {id}, command: {command:?}, revision: {revision} }}");
                Reply::Done {
                    server_time: now,
                    world: world.clone(),
                }
            }
            Err(reason) => {
                debug!("command refused {{ from: {from}, account: {id}, command: {command:?}, reason: {reason:?} }}");
                Reply::Refused {
                    server_time: now,
                    world: world.clone(),
                    reason,
                }
            }
        };

        let sealed = protocol::seal(&Packet::Reply { sequence, reply }, secret);
        let answer = Answer { tag, reply: &sealed };
        logged(self.store.save_command(account, order, &world, answer), "saving a command")?;
        Some(sealed)
    }

    /// no sequence check, two terminals syncing at once is fine
    fn sync(&self, account: &AccountId, since: Option<u64>, from: SocketAddr) -> Option<Reply> {
        let id = protocol::to_hex(account);
        let mut world = logged(self.store.world(account), "loading a world")?;
        let last_seen = world.last_seen();
        let now = now();
        world.catch_up(now);
        if world.last_seen() - last_seen >= SAVE_CAUGHT_UP_AFTER {
            logged(self.store.save_world(account, &world), "saving a world")?;
            let revision = world.revision();
            debug!("world saved {{ account: {id}, revision: {revision} }}");
        }

        let world = (since != Some(world.revision())).then_some(world);
        let sent = world.is_some();

        debug!("synced {{ from: {from}, account: {id}, since: {since:?}, world_sent: {sent} }}");

        Some(Reply::Synced { server_time: now, world })
    }
}

/// reply can never be bigger than request to prevent amplification
fn unsigned(packet: Packet, request: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
    let reply = protocol::encode(&packet);
    if reply.len() > request.len() {
        debug!(
            "answer dropped {{ to: {from}, reason: \"larger than the request\", request_bytes: {}, answer_bytes: {} }}",
            request.len(),
            reply.len()
        );
        return None;
    }
    Some(reply)
}

/// db error drops packet, server doesnt panic
fn logged<T>(result: std::result::Result<T, impl Display>, what: &str) -> Option<T> {
    result
        .map_err(|problem| error!("storage failed {{ during: {what:?}, error: {:?} }}", problem.to_string()))
        .ok()
}

fn now() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
}

fn minute() -> u64 {
    now().as_secs() / 60
}

fn flag(name: &str) -> Option<String> {
    let mut arguments = std::env::args().skip_while(|argument| argument != name);
    arguments.next()?;
    arguments.next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_counts_once_and_a_retry_gets_the_same_answer() {
        let server = Server {
            name: "test".to_string(),
            key: StaticSecret::from([1; 32]),
            challenge_secret: [2; 32],
            store: Store::open(Path::new(":memory:")).unwrap(),
        };
        let player = StaticSecret::from([3; 32]);
        let account = [4; 16];
        let world = World::new(Pet::new("Mochi", "Cat"), now());
        server
            .store
            .register(&account, PublicKey::from(&player).as_bytes(), &world)
            .unwrap();
        let from: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let secret = protocol::account_secret(&player, &PublicKey::from(&server.key));
        let feed = |sequence| {
            let request = Packet::Request {
                account,
                challenge: protocol::challenge(&server.challenge_secret, from, minute()),
                sequence,
                request: Request::Command {
                    command: Command::Act('f'),
                },
            };
            server.answer(&protocol::seal(&request, &secret), from)
        };
        let first = feed(5);
        assert!(first.is_some());
        assert_eq!(feed(5), first);
        assert_eq!(server.store.world(&account).unwrap().revision(), 1);
        assert!(feed(4).is_none());
        assert!(feed(6).is_some());
    }
}
