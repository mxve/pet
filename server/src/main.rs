mod store;

use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pet_core::pet::{self, Pet};
use pet_core::protocol::{self, Challenge, Info, MAX_PACKET, PORT, Packet, PublicKey, Reply, Secret, Signup, StaticSecret};
use pet_core::world::World;
use store::Store;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DEFAULT_DB: &str = "pet.db";
const DEFAULT_NAME: &str = "pet";

struct Server {
    name: String,
    key: StaticSecret,
    challenge_secret: Secret,
    store: Store,
}

fn main() -> Result<()> {
    let port = flag("--port").and_then(|port| port.parse().ok()).unwrap_or(PORT);
    let key = flag("--key").ok_or("pet-server needs --key <file>, made by scripts/keygen.sh")?;
    let database = PathBuf::from(flag("--db").unwrap_or_else(|| DEFAULT_DB.to_string()));
    let server = Server {
        name: flag("--name").unwrap_or_else(|| DEFAULT_NAME.to_string()),
        key: load_key(Path::new(&key))?,
        challenge_secret: protocol::random(),
        store: Store::open(&database).map_err(|error| format!("{}: {error}", database.display()))?,
    };
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    println!("listening on udp {port}, database {}", database.display());
    let mut buffer = [0; MAX_PACKET + 1];
    loop {
        let Ok((size, from)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        if let Some(reply) = server.answer(&buffer[..size], from) {
            socket.send_to(&reply, from).ok();
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
        match protocol::decode(request)? {
            Packet::GetInfo { .. } => unsigned(self.info()?, request),
            Packet::GetChallenge { nonce, .. } => unsigned(self.challenge(nonce, from), request),
            Packet::Register(signup) => self.register(signup, from),
            Packet::Info(_) | Packet::Challenge { .. } | Packet::Reply { .. } => None,
        }
    }

    fn info(&self) -> Option<Packet> {
        let info = Info {
            name: self.name.clone(),
            players: self.store.accounts().ok()?,
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
        let allowed = self.challenge_fits(&signup.challenge, from)
            && pet::valid_name(&signup.name)
            && pet::SPECIES.contains(&signup.species.as_str());
        if !allowed {
            return None;
        }
        let account = protocol::random();
        let now = now();
        let world = toml::to_string(&World::new(Pet::new(&signup.name, &signup.species), now)).ok()?;
        self.store.register(&account, &signup.public, now.as_secs() as i64, &world).ok()?;
        let secret = protocol::account_secret(&self.key, &PublicKey::from(signup.public));
        let reply = Packet::Reply {
            sequence: 0,
            reply: Reply::Registered { account },
        };
        Some(protocol::seal(&reply, &secret))
    }
}

fn unsigned(packet: Packet, request: &[u8]) -> Option<Vec<u8>> {
    let reply = protocol::encode(&packet);
    (reply.len() <= request.len()).then_some(reply)
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
