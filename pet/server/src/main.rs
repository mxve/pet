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
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    let public = protocol::to_hex(PublicKey::from(&server.key).as_bytes());
    let accounts = server.store.accounts().unwrap_or_default();
    info!("listening on udp {port}, database {}, {accounts} accounts", database.display());
    let shown = format!("{level:?}").to_lowercase();
    info!("public key {public}, logging {shown} and up to {}", log_file.display());
    let mut buffer = [0; MAX_PACKET + 1];
    loop {
        let (size, from) = match socket.recv_from(&mut buffer) {
            Ok(received) => received,
            Err(problem) => {
                warning!("receiving failed: {problem}");
                continue;
            }
        };
        if let Some(reply) = server.answer(&buffer[..size], from)
            && let Err(problem) = socket.send_to(&reply, from)
        {
            warning!("{from}: sending failed: {problem}");
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
                debug!("{from}: info");
                unsigned(self.info()?, request, from)
            }
            Packet::GetChallenge { nonce, .. } => {
                debug!("{from}: challenge");
                unsigned(self.challenge(nonce, from), request, from)
            }
            Packet::Register(signup) => self.register(signup, from),
            Packet::Info(_) | Packet::Challenge { .. } | Packet::Request { .. } | Packet::Reply { .. } => {
                debug!("{from}: dropped a packet only the server sends");
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
        let refusal = if !self.challenge_fits(&signup.challenge, from) {
            Some("a challenge that does not fit")
        } else if !pet::valid_name(&signup.name) {
            Some("an invalid name")
        } else if !pet::SPECIES.contains(&signup.species.as_str()) {
            Some("an unknown species")
        } else {
            None
        };
        if let Some(refusal) = refusal {
            debug!(
                "{from}: dropped a signup with {refusal} ({:?} the {:?})",
                signup.name, signup.species
            );
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
        info!("{from}: {} signed up {} the {}", short(&account), signup.name, signup.species);
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
            debug!("{from}: dropped {} bytes that are not a request", bytes.len());
            return None;
        };
        if !self.challenge_fits(&challenge, from) {
            debug!("{from}: {} sent a challenge that does not fit", short(&account));
            return None;
        }
        let Some(public) = logged(self.store.public_key(&account), "looking up an account")? else {
            debug!("{from}: {} is not an account", short(&account));
            return None;
        };
        let secret = protocol::account_secret(&self.key, &PublicKey::from(public));
        if protocol::open(bytes, &secret).is_none() {
            debug!("{from}: {} sent a request with a wrong tag", short(&account));
            return None;
        }
        let reply = match request {
            Request::Sync { since } => self.sync(&account, since)?,
        };
        Some(protocol::seal(&Packet::Reply { sequence, reply }, &secret))
    }

    fn sync(&self, account: &AccountId, since: Option<u64>) -> Option<Reply> {
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
            debug!("{} caught up and saved", short(account));
        }
        let world = (since != Some(world.revision())).then_some(world);
        let sent = if world.is_some() { "sent the world" } else { "unchanged" };
        debug!("{} synced, {sent}", short(account));
        Some(Reply::Synced { server_time: now, world })
    }
}

fn unsigned(packet: Packet, request: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
    let reply = protocol::encode(&packet);
    if reply.len() > request.len() {
        debug!(
            "{from}: dropped a {} byte request that would get a {} byte answer",
            request.len(),
            reply.len()
        );
        return None;
    }
    Some(reply)
}

fn logged<T>(result: std::result::Result<T, impl Display>, what: &str) -> Option<T> {
    result.map_err(|problem| error!("{what}: {problem}")).ok()
}

fn short(account: &AccountId) -> String {
    protocol::to_hex(&account[..4])
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
