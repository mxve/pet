use std::net::UdpSocket;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use pet_core::protocol::{
    self, AccountId, Challenge, Key, MAX_PACKET, PORT, Packet, PublicKey, Reply, Request, Secret, Signup, StaticSecret,
};
use pet_core::world::Command;

use crate::Result;
use crate::account::Account;

const SERVER: Option<&str> = option_env!("PET_SERVER");
const SERVER_KEY: Option<&str> = option_env!("PET_SERVER_KEY");
const TIMEOUT: Duration = Duration::from_secs(2);
const SYNC_EVERY: Duration = Duration::from_millis(250);
const CHALLENGE_LIFE: Duration = Duration::from_secs(60);
const RETRY_AFTER: Duration = Duration::from_secs(2);
const SECOND_COPY_AFTER: Duration = Duration::from_millis(30);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Remote {
    replies: Receiver<Reply>,
    commands: Sender<Command>,
}

impl Remote {
    pub fn received(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }

    pub fn send(&self, command: Command) {
        self.commands.send(command).ok();
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
    let (reply_sender, replies) = mpsc::channel();
    let (commands, command_receiver) = mpsc::channel();
    thread::spawn(move || keep_in_sync(&address, id, &secret, &command_receiver, &reply_sender));
    Ok(Some(Remote { replies, commands }))
}

fn server_key() -> Result<Key> {
    let key = SERVER_KEY.ok_or("built without PET_SERVER_KEY")?;
    Ok(protocol::from_hex(key).ok_or("PET_SERVER_KEY is not 64 hex characters")?)
}

fn keep_in_sync(address: &str, account: AccountId, secret: &Secret, commands: &Receiver<Command>, replies: &Sender<Reply>) {
    let mut since = None;
    let mut sequence = 0;
    let mut command = None;
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
            let commanding = command.is_some();
            let request = Packet::Request {
                account,
                challenge,
                sequence,
                request: match command.take() {
                    Some(command) => Request::Command { command },
                    None => Request::Sync { since },
                },
            };
            let sealed = protocol::seal(&request, secret);
            let answer = if commanding {
                server.insist(sequence, &sealed, secret)
            } else {
                server.ask(sequence, &sealed, secret)
            };
            let Ok(reply) = answer else {
                since = None;
                break;
            };
            if let Reply::Synced { world: Some(world), .. } | Reply::Done { world, .. } | Reply::Refused { world, .. } = &reply {
                since = Some(world.revision());
            }
            if replies.send(reply).is_err() {
                return;
            }
            match commands.recv_timeout(SYNC_EVERY.saturating_sub(asked.elapsed())) {
                Ok(next) => command = Some(next),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
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

    fn ask(&self, sequence: u64, request: &[u8], secret: &Secret) -> Result<Reply> {
        self.send(request)?;
        self.reply(sequence, secret)
    }

    fn insist(&self, sequence: u64, request: &[u8], secret: &Secret) -> Result<Reply> {
        let started = Instant::now();
        self.send(request)?;
        thread::sleep(SECOND_COPY_AFTER);
        loop {
            match self.ask(sequence, request, secret) {
                Err(_) if started.elapsed() < COMMAND_TIMEOUT => {}
                answer => return answer,
            }
        }
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
