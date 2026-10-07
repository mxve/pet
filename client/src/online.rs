use std::net::UdpSocket;
use std::time::Duration;

use pet_core::protocol::{self, Challenge, Key, MAX_PACKET, PORT, Packet, PublicKey, Reply, Signup, StaticSecret};

use crate::Result;
use crate::account::Account;

const SERVER: Option<&str> = option_env!("PET_SERVER");
const SERVER_KEY: Option<&str> = option_env!("PET_SERVER_KEY");
const TIMEOUT: Duration = Duration::from_secs(2);

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
    let (Some(address), Some(key)) = (SERVER, SERVER_KEY) else {
        return Ok(None);
    };
    let key = protocol::from_hex(key).ok_or("PET_SERVER_KEY is not 64 hex characters")?;
    let server = Server::connect(address)?;
    let challenge = server.challenge()?;
    server.register(challenge, key, name, species).map(Some)
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
        let Some(Packet::Reply {
            reply: Reply::Registered { account },
            ..
        }) = protocol::open(&self.receive_bytes()?, &secret)
        else {
            return Err(self.problem("answered without the server's signature"));
        };
        Ok(Account {
            server: self.address.clone(),
            id: protocol::to_hex(&account),
            key: protocol::to_hex(private.as_bytes()),
        })
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
