mod log;
mod store;

use std::fmt::Display;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{Level, debug, error, info, warning};
use pet_core::pet::{self, Pet};
use pet_core::protocol::{
    self, AccountId, Challenge, Info, MAX_PACKET, PORT, Packet, PublicKey, Reply, Request, Secret, Signup, StaticSecret,
};
use pet_core::world::World;
use store::Store;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DEFAULT_DB: &str = "pet.db";
const DEFAULT_NAME: &str = "pet";
const DEFAULT_LOG: &str = "pet-server.log";
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
    log::start(&log_file, level).map_err(|error| format!("{}: {error}", log_file.display()))?;
    let server = Server {
        name: flag("--name").unwrap_or_else(|| DEFAULT_NAME.to_string()),
        key: load_key(Path::new(&key))?,
        challenge_secret: protocol::random(),
        store: Store::open(&database).map_err(|error| format!("{}: {error}", database.display()))?,
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
    let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let bytes = protocol::from_hex(&text).ok_or_else(|| format!("{}: not 64 hex characters", path.display()))?;
    Ok(StaticSecret::from(bytes))
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

    fn challenge_fits(&self, challenge: &Challenge, from: SocketAddr) -> bool {
        let now = minute();
        [now, now.saturating_sub(1)]
            .into_iter()
            .any(|minute| protocol::challenge(&self.challenge_secret, from, minute) == *challenge)
    }

    fn register(&self, signup: Signup, from: SocketAddr) -> Option<Vec<u8>> {
        if !self.challenge_fits(&signup.challenge, from) {
            let (name, species) = (&signup.name, &signup.species);
            debug!("signup dropped {{ from: {from}, reason: \"challenge does not fit\", name: {name:?}, species: {species:?} }}");
            return None;
        }
        let refusal = if !pet::valid_name(&signup.name) {
            Some("invalid name")
        } else if !pet::SPECIES.contains(&signup.species.as_str()) {
            Some("unknown species")
        } else {
            None
        };
        if let Some(refusal) = refusal {
            let (name, species) = (&signup.name, &signup.species);
            info!("signup refused {{ from: {from}, reason: {refusal:?}, name: {name:?}, species: {species:?} }}");
            return None;
        }
        let account = protocol::random();
        let now = now();
        let world = World::new(Pet::new(&signup.name, &signup.species), now);
        let world = logged(toml::to_string(&world), "writing a new world")?;
        logged(
            self.store.register(&account, &signup.public, now.as_secs() as i64, &world),
            "storing a signup",
        )?;
        let (id, name, species) = (protocol::to_hex(&account), &signup.name, &signup.species);
        info!("signed up {{ from: {from}, account: {id}, name: {name:?}, species: {species:?} }}");
        let secret = protocol::account_secret(&self.key, &PublicKey::from(signup.public));
        let reply = Packet::Reply {
            sequence: 0,
            reply: Reply::Registered { account },
        };
        Some(protocol::seal(&reply, &secret))
    }

    fn signed(&self, bytes: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
        let Packet::Request {
            account,
            challenge,
            sequence,
            request,
        } = protocol::unverified(bytes)?
        else {
            debug!(
                "packet dropped {{ from: {from}, reason: \"not a packet\", bytes: {} }}",
                bytes.len()
            );
            return None;
        };
        if !self.challenge_fits(&challenge, from) {
            debug!(
                "request dropped {{ from: {from}, account: {}, reason: \"challenge does not fit\" }}",
                protocol::to_hex(&account)
            );
            return None;
        }
        let Some(public) = logged(self.store.public_key(&account), "looking up an account")? else {
            debug!(
                "login failed {{ from: {from}, account: {}, reason: \"no such account\" }}",
                protocol::to_hex(&account)
            );
            return None;
        };
        let secret = protocol::account_secret(&self.key, &PublicKey::from(public));
        if protocol::open(bytes, &secret).is_none() {
            debug!(
                "login failed {{ from: {from}, account: {}, reason: \"wrong signature\" }}",
                protocol::to_hex(&account)
            );
            return None;
        }
        let reply = match request {
            Request::Sync { since } => self.sync(&account, since, from)?,
        };
        Some(protocol::seal(&Packet::Reply { sequence, reply }, &secret))
    }

    fn sync(&self, account: &AccountId, since: Option<u64>, from: SocketAddr) -> Option<Reply> {
        let stored = logged(self.store.world(account), "loading a world")?;
        let mut world: World = logged(toml::from_str(&stored), "reading a stored world")?;
        let last_seen = world.last_seen();
        let now = now();
        world.catch_up(now);
        if world.last_seen() - last_seen >= SAVE_CAUGHT_UP_AFTER {
            let saved = logged(toml::to_string(&world), "writing a world")?;
            let revision = i64::try_from(world.revision()).ok()?;
            logged(
                self.store.save_world(account, revision, now.as_secs() as i64, &saved),
                "saving a world",
            )?;
            debug!("world saved {{ account: {}, revision: {revision} }}", protocol::to_hex(account));
        }
        let world = (since != Some(world.revision())).then_some(world);
        let (id, sent) = (protocol::to_hex(account), world.is_some());
        debug!("synced {{ from: {from}, account: {id}, since: {since:?}, world_sent: {sent} }}");
        Some(Reply::Synced { server_time: now, world })
    }
}

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
    fn a_world_survives_storage() {
        let world = World::new(Pet::new("Mochi", "Cat"), Duration::from_secs(1_759_000_000));
        let stored = toml::to_string(&world).unwrap();
        assert_eq!(toml::from_str::<World>(&stored).unwrap(), world);
    }
}
