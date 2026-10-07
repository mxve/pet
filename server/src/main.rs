mod store;

use std::net::UdpSocket;
use std::path::PathBuf;

use pet_core::protocol::{self, Info, MAX_PACKET, PORT, Packet};
use store::Store;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DEFAULT_DB: &str = "pet.db";

fn main() -> Result<()> {
    let port = flag("--port").and_then(|port| port.parse().ok()).unwrap_or(PORT);
    let name = flag("--name").unwrap_or_else(|| "pet".to_string());
    let path = PathBuf::from(flag("--db").unwrap_or_else(|| DEFAULT_DB.to_string()));
    let store = Store::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    println!("listening on udp {port}, database {}", path.display());
    let mut buffer = [0; MAX_PACKET + 1];
    loop {
        let Ok((size, from)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let request = &buffer[..size];
        if let Some(reply) = answer(request, &name, &store)
            && reply.len() <= request.len()
        {
            socket.send_to(&reply, from).ok();
        }
    }
}

fn answer(request: &[u8], name: &str, store: &Store) -> Option<Vec<u8>> {
    match protocol::decode(request)? {
        Packet::GetInfo { .. } => {
            let info = Info {
                name: name.to_string(),
                players: store.accounts().ok()?,
            };
            Some(protocol::encode(&Packet::Info(info)))
        }
        Packet::Info(_) => None,
    }
}

fn flag(name: &str) -> Option<String> {
    let mut arguments = std::env::args().skip_while(|argument| argument != name);
    arguments.next()?;
    arguments.next()
}
