use std::net::SocketAddr;

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use x25519_dalek::{PublicKey, StaticSecret};

pub const HEADER: [u8; 4] = [0xFF; 4];
pub const VERSION: u8 = 1;
pub const PORT: u16 = 27960;
pub const MAX_PACKET: usize = 1200;
const UNSIGNED_REQUEST_SIZE: usize = 128;
const TAG_SIZE: usize = 32;
const ACCOUNT_SECRET_LABEL: &[u8] = b"pet account v1";

pub type Key = [u8; 32];
pub type Secret = [u8; 32];
pub type Challenge = [u8; 16];
pub type AccountId = [u8; 16];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Packet {
    GetInfo { padding: Vec<u8> },
    Info(Info),
    GetChallenge { nonce: u64, padding: Vec<u8> },
    Challenge { nonce: u64, challenge: Challenge },
    Register(Signup),
    Reply { sequence: u64, reply: Reply },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub players: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signup {
    pub challenge: Challenge,
    pub public: Key,
    pub name: String,
    pub species: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Reply {
    Registered { account: AccountId },
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
    padded(|padding| Packet::GetInfo { padding })
}

pub fn get_challenge(nonce: u64) -> Vec<u8> {
    padded(|padding| Packet::GetChallenge { nonce, padding })
}

fn padded(packet: impl Fn(Vec<u8>) -> Packet) -> Vec<u8> {
    let bare = encode(&packet(Vec::new()));
    encode(&packet(vec![0; UNSIGNED_REQUEST_SIZE.saturating_sub(bare.len() + 1)]))
}

pub fn seal(packet: &Packet, secret: &Secret) -> Vec<u8> {
    let mut bytes = encode(packet);
    let tag = mac(secret).chain_update(&bytes).finalize().into_bytes();
    bytes.extend_from_slice(&tag);
    bytes
}

pub fn open(bytes: &[u8], secret: &Secret) -> Option<Packet> {
    let (body, tag) = bytes.split_at_checked(bytes.len().checked_sub(TAG_SIZE)?)?;
    mac(secret).chain_update(body).verify_slice(tag).ok()?;
    decode(body)
}

pub fn account_secret(own: &StaticSecret, other: &PublicKey) -> Secret {
    let shared = own.diffie_hellman(other);
    Sha256::new()
        .chain_update(ACCOUNT_SECRET_LABEL)
        .chain_update(shared.as_bytes())
        .finalize()
        .into()
}

pub fn challenge(secret: &Secret, address: SocketAddr, minute: u64) -> Challenge {
    let tag = mac(secret)
        .chain_update(address.to_string())
        .chain_update(minute.to_le_bytes())
        .finalize()
        .into_bytes();
    tag[..16].try_into().expect("an HMAC-SHA256 tag is 32 bytes")
}

pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system provides randomness");
    bytes
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != 2 * N {
        return None;
    }
    let mut bytes = [0; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * index..2 * index + 2)?, 16).ok()?;
    }
    Some(bytes)
}

fn mac(key: &[u8]) -> Hmac<Sha256> {
    Hmac::new_from_slice(key).expect("HMAC takes a key of any length")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_whole_framed_packets_decode() {
        let info = Packet::Info(Info {
            name: "pet".into(),
            players: 3,
        });
        let bytes = encode(&info);
        assert_eq!(decode(&bytes), Some(info));
        assert_eq!(decode(&bytes[4..]), None);
        assert_eq!(decode(&[&bytes[..], &[0]].concat()), None);
        assert_eq!(decode(&vec![0xFF; MAX_PACKET + 1]), None);
    }

    #[test]
    fn a_sealed_packet_opens_only_with_its_secret_and_untouched() {
        let packet = Packet::Reply {
            sequence: 7,
            reply: Reply::Registered { account: [1; 16] },
        };
        let sealed = seal(&packet, &[1; 32]);
        assert_eq!(open(&sealed, &[1; 32]), Some(packet));
        assert_eq!(open(&sealed, &[2; 32]), None);
        let mut changed = sealed.clone();
        changed[10] ^= 1;
        assert_eq!(open(&changed, &[1; 32]), None);
    }

    #[test]
    fn a_challenge_fits_only_its_address_and_minute() {
        let home: SocketAddr = "10.0.0.1:5000".parse().unwrap();
        let elsewhere: SocketAddr = "10.0.0.2:5000".parse().unwrap();
        let made = challenge(&[3; 32], home, 100);
        assert_eq!(challenge(&[3; 32], home, 100), made);
        assert_ne!(challenge(&[3; 32], elsewhere, 100), made);
        assert_ne!(challenge(&[3; 32], home, 101), made);
    }
}
