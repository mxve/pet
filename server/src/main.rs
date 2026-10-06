use std::net::UdpSocket;

use pet_core::protocol::{self, Info, MAX_PACKET, PORT, Packet};

fn main() -> std::io::Result<()> {
    let port = flag("--port").and_then(|port| port.parse().ok()).unwrap_or(PORT);
    let info = Info {
        name: flag("--name").unwrap_or_else(|| "pet".to_string()),
    };
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    println!("listening on udp {port}");
    let mut buffer = [0; MAX_PACKET + 1];
    loop {
        let Ok((size, from)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let request = &buffer[..size];
        if let Some(reply) = answer(request, &info)
            && reply.len() <= request.len()
        {
            socket.send_to(&reply, from).ok();
        }
    }
}

fn answer(request: &[u8], info: &Info) -> Option<Vec<u8>> {
    match protocol::decode(request)? {
        Packet::GetInfo { .. } => Some(protocol::encode(&Packet::Info(info.clone()))),
        Packet::Info(_) => None,
    }
}

fn flag(name: &str) -> Option<String> {
    let mut arguments = std::env::args().skip_while(|argument| argument != name);
    arguments.next()?;
    arguments.next()
}
