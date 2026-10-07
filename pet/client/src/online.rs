use std::net::UdpSocket;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use pet_core::protocol::{
    self, AccountId, Challenge, Key, MAX_PACKET, PORT, Packet, PublicKey, Reply, Request, Secret, Signup, StaticSecret,
};
use pet_core::world::World;

use crate::Result;
use crate::account::Account;

const SERVER: Option<&str> = option_env!("PET_SERVER");
const SERVER_KEY: Option<&str> = option_env!("PET_SERVER_KEY");
const TIMEOUT: Duration = Duration::from_secs(2);
const SYNC_EVERY: Duration = Duration::from_millis(250);
const CHALLENGE_LIFE: Duration = Duration::from_secs(60);
const RETRY_AFTER: Duration = Duration::from_secs(2);

pub struct Synced {
    pub server_time: Duration,
    pub world: Option<World>,
}

pub struct Remote {
    synced: Receiver<Synced>,
}

impl Remote {
    pub fn received(&self) -> Vec<Synced> {
        self.synced.try_iter().collect()
    }
}

pub fn ping(address: &str) -> Result<()> {
    let server = Server::connect(address)?;
    server.send(&protocol::get_info())?;
    let Packet::Info(info) = server.receive()? else {
        return Err(server.problem("answered with something that is not info"));
    };
    println!(
        "{}: {}, {} players (protocol {})",
        server.address,
        info.name,
        info.players,
        protocol::VERSION
    );
    Ok(())
}

pub fn register(name: &str, species: &str) -> Result<Option<Account>> {
    let (Some(address), Some(_)) = (SERVER, SERVER_KEY) else {
        return Ok(None);
    };
    let server = Server::connect(address)?;
    let challenge = server.challenge()?;
    server.register(challenge, server_key()?, name, species).map(Some)
}

pub fn follow(account: &Account) -> Result<Option<Remote>> {
    if SERVER_KEY.is_none() {
        return Ok(None);
    }
    let id = protocol::from_hex(&account.id).ok_or("account.toml: the id is not 32 hex characters")?;
    let private = protocol::from_hex(&account.key).ok_or("account.toml: the key is not 64 hex characters")?;
    let secret = protocol::account_secret(&StaticSecret::from(private), &PublicKey::from(server_key()?));
    let address = account.server.clone();
    let (sender, synced) = mpsc::channel();
    thread::spawn(move || keep_in_sync(&address, id, &secret, &sender));
    Ok(Some(Remote { synced }))
}

fn server_key() -> Result<Key> {
    let key = SERVER_KEY.ok_or("built without PET_SERVER_KEY")?;
    Ok(protocol::from_hex(key).ok_or("PET_SERVER_KEY is not 64 hex characters")?)
}

fn keep_in_sync(address: &str, account: AccountId, secret: &Secret, sender: &Sender<Synced>) {
    let mut since = None;
    let mut sequence = 0;
    loop {
        let Ok(server) = Server::connect(address) else {
            thread::sleep(RETRY_AFTER);
            continue;
        };
        let Ok(challenge) = server.challenge() else {
            continue;
        };
        server.socket.set_read_timeout(Some(SYNC_EVERY)).ok();
        let fetched = Instant::now();
        while fetched.elapsed() < CHALLENGE_LIFE {
            let asked = Instant::now();
            sequence = (sequence + 1).max(crate::now().as_millis() as u64);
            let request = Packet::Request {
                account,
                challenge,
                sequence,
                request: Request::Sync { since },
            };
            let Ok(Reply::Synced { server_time, world }) = server.ask(sequence, &request, secret) else {
                break;
            };
            since = world.as_ref().map(World::revision).or(since);
            if sender.send(Synced { server_time, world }).is_err() {
                return;
            }
            thread::sleep(SYNC_EVERY.saturating_sub(asked.elapsed()));
        }
    }
}

struct Server {
    address: String,
    socket: UdpSocket,
}

impl Server {
    fn connect(address: &str) -> Result<Server> {
        let address = if address.contains(':') {
            address.to_string()
        } else {
            format!("{address}:{PORT}")
        };
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.connect(&address)?;
        Ok(Server { address, socket })
    }

    fn challenge(&self) -> Result<Challenge> {
        let nonce = u64::from_le_bytes(protocol::random());
        self.send(&protocol::get_challenge(nonce))?;
        match self.receive()? {
            Packet::Challenge { nonce: echoed, challenge } if echoed == nonce => Ok(challenge),
            _ => Err(self.problem("sent no challenge")),
        }
    }

    fn register(&self, challenge: Challenge, key: Key, name: &str, species: &str) -> Result<Account> {
        let private = StaticSecret::from(protocol::random::<32>());
        self.send(&protocol::encode(&Packet::Register(Signup {
            challenge,
            public: PublicKey::from(&private).to_bytes(),
            name: name.to_string(),
            species: species.to_string(),
        })))?;
        let secret = protocol::account_secret(&private, &PublicKey::from(key));
        let Reply::Registered { account } = self.reply(0, &secret)? else {
            return Err(self.problem("answered something other than the signup"));
        };
        Ok(Account {
            server: self.address.clone(),
            id: protocol::to_hex(&account),
            key: protocol::to_hex(private.as_bytes()),
        })
    }

    fn ask(&self, sequence: u64, request: &Packet, secret: &Secret) -> Result<Reply> {
        self.send(&protocol::seal(request, secret))?;
        self.reply(sequence, secret)
    }

    fn reply(&self, sequence: u64, secret: &Secret) -> Result<Reply> {
        loop {
            let Some(Packet::Reply { sequence: answered, reply }) = protocol::open(&self.receive_bytes()?, secret) else {
                return Err(self.problem("answered without the server's signature"));
            };
            if answered == sequence {
                return Ok(reply);
            }
        }
    }

    fn send(&self, bytes: &[u8]) -> Result<()> {
        self.socket.send(bytes)?;
        Ok(())
    }

    fn receive(&self) -> Result<Packet> {
        protocol::decode(&self.receive_bytes()?).ok_or_else(|| self.problem("answered with something that does not decode"))
    }

    fn receive_bytes(&self) -> Result<Vec<u8>> {
        let mut buffer = [0; MAX_PACKET + 1];
        let size = self
            .socket
            .recv(&mut buffer)
            .map_err(|error| self.problem(&format!("did not answer ({error})")))?;
        Ok(buffer[..size].to_vec())
    }

    fn problem(&self, what: &str) -> Box<dyn std::error::Error> {
        format!("{}: {what}", self.address).into()
    }
}
