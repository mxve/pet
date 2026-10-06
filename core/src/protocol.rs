use serde::{Deserialize, Serialize};

pub const HEADER: [u8; 4] = [0xFF; 4];
pub const VERSION: u8 = 1;
pub const PORT: u16 = 27960;
pub const MAX_PACKET: usize = 1200;
pub const GET_INFO_SIZE: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Packet {
    GetInfo { padding: Vec<u8> },
    Info(Info),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
}

pub fn encode(packet: &Packet) -> Vec<u8> {
    let body = postcard::to_stdvec(packet).expect("packets always encode");
    [&HEADER[..], &[VERSION], &body].concat()
}

pub fn decode(bytes: &[u8]) -> Option<Packet> {
    if bytes.len() > MAX_PACKET {
        return None;
    }
    let body = bytes.strip_prefix(&HEADER)?.strip_prefix(&[VERSION])?;
    match postcard::take_from_bytes(body) {
        Ok((packet, [])) => Some(packet),
        _ => None,
    }
}

pub fn get_info() -> Vec<u8> {
    let bare = encode(&Packet::GetInfo { padding: Vec::new() });
    let padding = vec![0; GET_INFO_SIZE.saturating_sub(bare.len() + 1)];
    encode(&Packet::GetInfo { padding })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_whole_framed_packets_decode() {
        let info = Packet::Info(Info { name: "pet".into() });
        let bytes = encode(&info);
        assert_eq!(decode(&bytes), Some(info));
        assert_eq!(decode(&bytes[4..]), None);
        assert_eq!(decode(&[&bytes[..], &[0]].concat()), None);
        assert_eq!(decode(&vec![0xFF; MAX_PACKET + 1]), None);
    }
}
