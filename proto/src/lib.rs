#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

//! Shared protocol types for the wireless keyboard system.
//! Keeps host-side simulation and firmware aligned on message layouts, timing, and validation rules.

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

#[cfg(feature = "std")]
use std::time::Duration;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use core::time::Duration;

mod aead;
pub mod backend;
pub mod sim;
pub use aead::{Aead, CryptoError, DummyAead};
#[cfg(feature = "crypto")]
pub use aead::RealAead;
#[cfg(not(feature = "crypto"))]
pub use aead::DummyAead as DefaultAead;
#[cfg(feature = "crypto")]
pub use aead::RealAead as DefaultAead;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::vec;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::vec::Vec as StdVec;

pub const MAX_MAC_BYTES: usize = 16;
pub const KEY_BYTES: usize = 32;
pub const NONCE_BYTES: usize = 24; // XChaCha20-Poly1305 nonce size
pub const SESSION_SALT_BYTES: usize = 16;
pub const MAX_RETRANSMIT_ATTEMPTS: u8 = 1; // single retry, no backoff, to bound latency
pub const HEADER_LEN: usize = 10; // session_id (4) + counter (4) + kind (1) + flags (1)
pub const AAD_LEN: usize = HEADER_LEN + 2; // header + payload length (u16 LE)

#[cfg(all(not(feature = "std"), feature = "alloc"))]
pub(crate) type Vec<T> = StdVec<T>;
#[cfg(feature = "std")]
pub(crate) type Vec<T> = std::vec::Vec<T>;

#[derive(Clone, Copy, Debug)]
pub struct WakeTiming {
    pub idle_sleep: Duration,
    pub listen_window: Duration,
    pub reconnect_timeout: Duration,
}

#[derive(Clone, Copy, Debug)]
pub enum CipherSuite {
    XChaCha20Poly1305,
}

#[derive(Clone, Copy, Debug)]
pub enum HandshakeKind {
    NoiseX25519,
    PreShared,
}

#[derive(Clone, Copy, Debug)]
pub struct SecurityConfig {
    pub handshake: HandshakeKind,
    pub forward_secure: bool,
    pub replay_protection: bool,
    pub cipher_suite: CipherSuite,
    pub mac_len: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct LatencyBudget {
    pub target: Duration,
    pub max: Duration,
}

#[derive(Clone, Copy, Debug)]
pub struct ProtocolConfig {
    pub wake: WakeTiming,
    pub security: SecurityConfig,
    pub latency: LatencyBudget,
    pub max_payload_bytes: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketKind {
    Handshake,
    Control,
    KeyReport,
    Ack,
    KeepAlive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketFlags {
    pub encrypted: bool,
    pub needs_ack: bool,
    pub retransmit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketHeader {
    pub session_id: u32,
    pub counter: u32,
    pub kind: PacketKind,
    pub flags: PacketFlags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    HandshakeInit { eph_pubkey: [u8; KEY_BYTES], nonce: [u8; NONCE_BYTES] },
    HandshakeAccept { session_id: u32 },
    Control { code: u8, data: Vec<u8> },
    KeyReport { keys: Vec<u8> },
    Ack { ack_counter: u32 },
    KeepAlive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Payload,
    /// Authentication tag; length dictated by `SecurityConfig::mac_len`.
    pub mac: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidationError {
    MissingMac,
    ReplayDetected,
    PayloadTooLarge,
    CounterJump,
    SessionMismatch,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SerializationError {
    PayloadTooLarge,
    MacLengthMismatch,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnexpectedLength,
    UnknownKind(u8),
    MacLengthMismatch,
}

pub fn validate_packet(
    packet: &Packet,
    cfg: &ProtocolConfig,
    expected_session: Option<u32>,
    last_counter: Option<u32>,
) -> Result<(), ValidationError> {
    if packet.mac.is_empty() || packet.mac.len() != cfg.security.mac_len {
        return Err(ValidationError::MissingMac);
    }

    if let Some(session) = expected_session {
        if packet.header.session_id != session {
            return Err(ValidationError::SessionMismatch);
        }
    }

    if let Some(last) = last_counter {
        if packet.header.counter == last {
            return Err(ValidationError::ReplayDetected);
        }
        if packet.header.counter < last {
            return Err(ValidationError::CounterJump);
        }
        if packet.header.counter - last > 50 {
            return Err(ValidationError::CounterJump);
        }
    }

    let payload_len = payload_len(&packet.payload);
    if payload_len > payload_limit(packet.header.kind, cfg) {
        return Err(ValidationError::PayloadTooLarge);
    }

    Ok(())
}

fn payload_len(payload: &Payload) -> usize {
    match payload {
        Payload::HandshakeInit { .. } => KEY_BYTES + NONCE_BYTES,
        Payload::HandshakeAccept { .. } => 4,
        Payload::Control { data, .. } => 1 + data.len(),
        Payload::KeyReport { keys } => keys.len(),
        Payload::Ack { .. } => 4,
        Payload::KeepAlive => 0,
    }
}

/// Encode the header to a fixed-width byte array (LE fields).
pub fn encode_header(header: &PacketHeader) -> [u8; HEADER_LEN] {
    let mut out = [0u8; HEADER_LEN];
    out[..4].copy_from_slice(&header.session_id.to_le_bytes());
    out[4..8].copy_from_slice(&header.counter.to_le_bytes());
    out[8] = header.kind as u8;
    out[9] = flags_to_byte(&header.flags);
    out
}

/// Associated data for MAC: header bytes + payload length (u16 LE).
pub fn associated_data(header: &PacketHeader, payload_len: usize) -> [u8; AAD_LEN] {
    let mut out = [0u8; AAD_LEN];
    out[..HEADER_LEN].copy_from_slice(&encode_header(header));
    out[HEADER_LEN..AAD_LEN].copy_from_slice(&(payload_len as u16).to_le_bytes());
    out
}

/// Encode payload into the on-wire representation (no MAC/encryption).
pub fn encode_payload(payload: &Payload) -> Vec<u8> {
    match payload {
        Payload::HandshakeInit { eph_pubkey, nonce } => {
            let mut buf = Vec::with_capacity(KEY_BYTES + NONCE_BYTES);
            buf.extend_from_slice(eph_pubkey);
            buf.extend_from_slice(nonce);
            buf
        }
        Payload::HandshakeAccept { session_id } => session_id.to_le_bytes().to_vec(),
        Payload::Control { code, data } => {
            let mut buf = Vec::with_capacity(1 + data.len());
            buf.push(*code);
            buf.extend_from_slice(data);
            buf
        }
        Payload::KeyReport { keys } => keys.clone(),
        Payload::Ack { ack_counter } => ack_counter.to_le_bytes().to_vec(),
        Payload::KeepAlive => Vec::new(),
    }
}

/// Serialize a packet into (header bytes, payload bytes, associated data) with basic checks.
pub fn serialize_packet(
    packet: &Packet,
    cfg: &ProtocolConfig,
) -> Result<SerializedPacket, SerializationError> {
    let payload = encode_payload(&packet.payload);
    if payload.len() > payload_limit(packet.header.kind, cfg) {
        return Err(SerializationError::PayloadTooLarge);
    }

    if packet.mac.len() != cfg.security.mac_len {
        return Err(SerializationError::MacLengthMismatch);
    }

    let header_bytes = encode_header(&packet.header);
    let aad = associated_data(&packet.header, payload.len());

    Ok(SerializedPacket {
        header: header_bytes,
        payload,
        mac: packet.mac.clone(),
        aad,
    })
}

pub struct SerializedPacket {
    pub header: [u8; HEADER_LEN],
    pub payload: Vec<u8>,
    pub mac: Vec<u8>,
    pub aad: [u8; AAD_LEN],
}

/// Header || payload_len (u16 LE) || payload || mac
pub fn serialize_framed(packet: &Packet, cfg: &ProtocolConfig) -> Result<Vec<u8>, SerializationError> {
    let serialized = serialize_packet(packet, cfg)?;
    let mut out = Vec::with_capacity(
        HEADER_LEN + 2 + serialized.payload.len() + serialized.mac.len(),
    );
    out.extend_from_slice(&serialized.header);
    out.extend_from_slice(&(serialized.payload.len() as u16).to_le_bytes());
    out.extend_from_slice(&serialized.payload);
    out.extend_from_slice(&serialized.mac);
    Ok(out)
}

pub fn decode_header(bytes: &[u8]) -> Result<PacketHeader, ParseError> {
    if bytes.len() != HEADER_LEN {
        return Err(ParseError::UnexpectedLength);
    }

    let session_id = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let counter = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let kind = match bytes[8] {
        0 => PacketKind::Handshake,
        1 => PacketKind::Control,
        2 => PacketKind::KeyReport,
        3 => PacketKind::Ack,
        4 => PacketKind::KeepAlive,
        other => return Err(ParseError::UnknownKind(other)),
    };
    let flags = flags_from_byte(bytes[9]);

    Ok(PacketHeader {
        session_id,
        counter,
        kind,
        flags,
    })
}

pub fn decode_payload(kind: PacketKind, bytes: &[u8]) -> Result<Payload, ParseError> {
    match kind {
        PacketKind::Handshake => {
            if bytes.len() == KEY_BYTES + NONCE_BYTES {
                let mut key = [0u8; KEY_BYTES];
                let mut nonce = [0u8; NONCE_BYTES];
                key.copy_from_slice(&bytes[..KEY_BYTES]);
                nonce.copy_from_slice(&bytes[KEY_BYTES..]);
                Ok(Payload::HandshakeInit {
                    eph_pubkey: key,
                    nonce,
                })
            } else if bytes.len() == 4 {
                let session_id = u32::from_le_bytes(bytes.try_into().unwrap());
                Ok(Payload::HandshakeAccept { session_id })
            } else {
                Err(ParseError::UnexpectedLength)
            }
        }
        PacketKind::Control => {
            if bytes.is_empty() {
                return Err(ParseError::UnexpectedLength);
            }
            let code = bytes[0];
            let data = bytes[1..].to_vec();
            Ok(Payload::Control { code, data })
        }
        PacketKind::KeyReport => Ok(Payload::KeyReport {
            keys: bytes.to_vec(),
        }),
        PacketKind::Ack => {
            if bytes.len() != 4 {
                return Err(ParseError::UnexpectedLength);
            }
            let ack_counter = u32::from_le_bytes(bytes.try_into().unwrap());
            Ok(Payload::Ack { ack_counter })
        }
        PacketKind::KeepAlive => {
            if !bytes.is_empty() {
                return Err(ParseError::UnexpectedLength);
            }
            Ok(Payload::KeepAlive)
        }
    }
}

pub fn parse_packet(
    header_bytes: &[u8],
    payload_bytes: &[u8],
    mac_bytes: &[u8],
    cfg: &ProtocolConfig,
) -> Result<Packet, ParseError> {
    if mac_bytes.len() != cfg.security.mac_len {
        return Err(ParseError::MacLengthMismatch);
    }

    let header = decode_header(header_bytes)?;
    if payload_bytes.len() > payload_limit(header.kind, cfg) {
        return Err(ParseError::UnexpectedLength);
    }
    let payload = decode_payload(header.kind, payload_bytes)?;

    Ok(Packet {
        header,
        payload,
        mac: mac_bytes.to_vec(),
    })
}

/// Parse from header || payload_len (u16 LE) || payload || mac framing.
pub fn parse_framed(bytes: &[u8], cfg: &ProtocolConfig) -> Result<Packet, ParseError> {
    if bytes.len() < HEADER_LEN + 2 + cfg.security.mac_len {
        return Err(ParseError::UnexpectedLength);
    }

    let header_bytes = &bytes[..HEADER_LEN];
    let payload_len =
        u16::from_le_bytes(bytes[HEADER_LEN..HEADER_LEN + 2].try_into().unwrap()) as usize;
    let payload_start = HEADER_LEN + 2;
    let payload_end = payload_start + payload_len;
    if payload_end > bytes.len() {
        return Err(ParseError::UnexpectedLength);
    }

    let mac_start = payload_end;
    let mac_len = bytes.len().saturating_sub(mac_start);
    if mac_len != cfg.security.mac_len {
        return Err(ParseError::MacLengthMismatch);
    }

    let payload_bytes = &bytes[payload_start..payload_end];
    let mac_bytes = &bytes[mac_start..];

    parse_packet(header_bytes, payload_bytes, mac_bytes, cfg)
}

/// Seal a packet and frame it (header || len || ciphertext || mac) using the provided AEAD.
/// Placeholder: ciphertext may equal plaintext depending on the AEAD implementation.
pub fn seal_framed(
    packet: &Packet,
    cfg: &ProtocolConfig,
    aead: &dyn Aead,
    nonce: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let serialized = serialize_packet(packet, cfg).map_err(CryptoError::Serialize)?;
    let (ciphertext, mac) =
        aead.seal(nonce, &serialized.aad, &serialized.payload, cfg.security.mac_len)?;

    let mut out =
        Vec::with_capacity(HEADER_LEN + 2 + ciphertext.len() + cfg.security.mac_len);
    out.extend_from_slice(&serialized.header);
    out.extend_from_slice(&(ciphertext.len() as u16).to_le_bytes());
    out.extend_from_slice(&ciphertext);
    out.extend_from_slice(&mac);
    Ok(out)
}

/// Parse and authenticate a framed packet. Payload is returned as-is from the AEAD (plaintext if no encryption).
pub fn open_framed(
    bytes: &[u8],
    cfg: &ProtocolConfig,
    aead: &dyn Aead,
    nonce: &[u8],
) -> Result<Packet, CryptoError> {
    if bytes.len() < HEADER_LEN + 2 + cfg.security.mac_len {
        return Err(CryptoError::Parse(ParseError::UnexpectedLength));
    }

    let header_bytes = &bytes[..HEADER_LEN];
    let header = decode_header(header_bytes).map_err(CryptoError::Parse)?;

    let payload_len =
        u16::from_le_bytes(bytes[HEADER_LEN..HEADER_LEN + 2].try_into().unwrap()) as usize;
    let payload_start = HEADER_LEN + 2;
    let payload_end = payload_start + payload_len;
    if payload_end > bytes.len() {
        return Err(CryptoError::Parse(ParseError::UnexpectedLength));
    }

    let mac_start = payload_end;
    let mac_len = bytes.len().saturating_sub(mac_start);
    if mac_len != cfg.security.mac_len {
        return Err(CryptoError::Parse(ParseError::MacLengthMismatch));
    }

    let payload_bytes = &bytes[payload_start..payload_end];
    let mac_bytes = &bytes[mac_start..];

    if payload_bytes.len() > payload_limit(header.kind, cfg) {
        return Err(CryptoError::Parse(ParseError::UnexpectedLength));
    }

    let aad = associated_data(&header, payload_bytes.len());
    let plaintext = aead
        .open(nonce, &aad, payload_bytes, mac_bytes)
        .map_err(|e| e)?;

    let payload = decode_payload(header.kind, &plaintext).map_err(CryptoError::Parse)?;

    Ok(Packet {
        header,
        payload,
        mac: mac_bytes.to_vec(),
    })
}

/// Session-scoped keys and counters; session reset implies counter reset.
pub struct SessionKeys {
    pub session_id: u32,
    pub salt: [u8; SESSION_SALT_BYTES],
    counter: u32,
}

impl SessionKeys {
    pub fn new(session_id: u32, salt: [u8; SESSION_SALT_BYTES]) -> Self {
        Self {
            session_id,
            salt,
            counter: 1, // first data packet after handshake
        }
    }

    /// Counter used for handshake auth; fixed to zero for deterministic nonce derivation.
    pub fn handshake_nonce(&self) -> [u8; NONCE_BYTES] {
        derive_nonce(&self.salt, 0)
    }

    /// Retrieve and increment the counter for the next packet in this session.
    pub fn next_counter(&mut self) -> u32 {
        let current = self.counter;
        self.counter = self.counter.saturating_add(1);
        current
    }

    /// Reset counters when a session is rekeyed/restarted.
    pub fn reset_counter(&mut self) {
        self.counter = 1;
    }

    /// Resume from a persisted next counter (e.g., warm wake with cached session).
    pub fn resume_from(&mut self, next_counter: u32) {
        self.counter = next_counter.max(1);
    }

    /// Deterministic nonce derivation: session_salt || counter (LE).
    pub fn nonce_for(&self, counter: u32) -> [u8; NONCE_BYTES] {
        derive_nonce(&self.salt, counter)
    }
}

pub fn derive_nonce(session_salt: &[u8; SESSION_SALT_BYTES], counter: u32) -> [u8; NONCE_BYTES] {
    let mut out = [0u8; NONCE_BYTES];
    out[..SESSION_SALT_BYTES].copy_from_slice(session_salt);
    out[SESSION_SALT_BYTES..SESSION_SALT_BYTES + 4].copy_from_slice(&counter.to_le_bytes());
    out
}

#[derive(Clone, Debug)]
pub enum SimEvent {
    KeyboardWakes,
    LinkAuthenticated,
    KeyReport { keys: Vec<u8> },
    Idle,
}

#[derive(Clone, Debug)]
pub struct SimFrame {
    pub t_ms: u64,
    pub event: SimEvent,
}

pub fn demo_config() -> ProtocolConfig {
    ProtocolConfig {
        wake: WakeTiming {
            idle_sleep: Duration::from_millis(200),
            listen_window: Duration::from_millis(8),
            reconnect_timeout: Duration::from_millis(2),
        },
        security: SecurityConfig {
            handshake: HandshakeKind::NoiseX25519,
            forward_secure: true,
            replay_protection: true,
            cipher_suite: CipherSuite::XChaCha20Poly1305,
            mac_len: MAX_MAC_BYTES,
        },
        latency: LatencyBudget {
            target: Duration::from_millis(6),
            max: Duration::from_millis(10),
        },
        max_payload_bytes: 32,
    }
}

/// Build a high-level wake/auth/key delivery timeline for quick iteration in host-side sims.
pub fn simulate_wake_sequence(cfg: &ProtocolConfig) -> Vec<SimFrame> {
    let mut t = 0;
    let mut frames = Vec::new();

    frames.push(SimFrame {
        t_ms: t,
        event: SimEvent::KeyboardWakes,
    });

    t += cfg.wake.listen_window.as_millis() as u64;
    frames.push(SimFrame {
        t_ms: t,
        event: SimEvent::LinkAuthenticated,
    });

    t += cfg.latency.target.as_millis() as u64;
    frames.push(SimFrame {
        t_ms: t,
        event: SimEvent::KeyReport { keys: vec![0x04] }, // example: key 'a'
    });

    t += cfg.wake.idle_sleep.as_millis() as u64;
    frames.push(SimFrame {
        t_ms: t,
        event: SimEvent::Idle,
    });

    frames
}

/// Construct sample packets showing the expected header/payload/MAC makeup.
pub fn sample_packets(cfg: &ProtocolConfig) -> Vec<Packet> {
    let session_id = 0x88_77_66_55;
    let mut session = SessionKeys::new(session_id, [0xA5; SESSION_SALT_BYTES]);

    let handshake = Packet {
        header: PacketHeader {
            session_id,
            counter: 0,
            kind: PacketKind::Handshake,
            flags: PacketFlags {
                encrypted: false,
                needs_ack: true,
                retransmit: false,
            },
        },
        payload: Payload::HandshakeInit {
            eph_pubkey: [0xAA; KEY_BYTES],
            nonce: session.handshake_nonce(),
        },
        mac: vec![0x11; cfg.security.mac_len],
    };

    let counter = session.next_counter();
    let key_report = Packet {
        header: PacketHeader {
            session_id,
            counter,
            kind: PacketKind::KeyReport,
            flags: PacketFlags {
                encrypted: true,
                needs_ack: true,
                retransmit: false,
            },
        },
        payload: Payload::KeyReport { keys: vec![0x04] },
        mac: vec![0x22; cfg.security.mac_len],
    };

    let counter = session.next_counter();
    let ack = Packet {
        header: PacketHeader {
            session_id,
            counter,
            kind: PacketKind::Ack,
            flags: PacketFlags {
                encrypted: true,
                needs_ack: false,
                retransmit: false,
            },
        },
        payload: Payload::Ack { ack_counter: key_report.header.counter },
        mac: vec![0x33; cfg.security.mac_len],
    };

    vec![handshake, key_report, ack]
}

fn flags_to_byte(flags: &PacketFlags) -> u8 {
    (flags.encrypted as u8)
        | ((flags.needs_ack as u8) << 1)
        | ((flags.retransmit as u8) << 2)
}

fn flags_from_byte(b: u8) -> PacketFlags {
    PacketFlags {
        encrypted: b & 0x01 != 0,
        needs_ack: b & 0x02 != 0,
        retransmit: b & 0x04 != 0,
    }
}

fn payload_limit(kind: PacketKind, cfg: &ProtocolConfig) -> usize {
    match kind {
        PacketKind::Handshake => KEY_BYTES + NONCE_BYTES, // handshake can exceed data payload cap
        _ => cfg.max_payload_bytes as usize,
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn serialize_roundtrip_sample_packets() {
        let cfg = demo_config();
        let mut packets = sample_packets(&cfg);
        packets.push(handshake_accept_sample(&cfg));

        for pkt in packets {
            let serialized = serialize_packet(&pkt, &cfg).expect("serialize");
            let parsed = parse_packet(
                &serialized.header,
                &serialized.payload,
                &serialized.mac,
                &cfg,
            )
            .expect("parse");

            assert_eq!(pkt.header, parsed.header);
            assert_eq!(pkt.payload, parsed.payload);
            assert_eq!(pkt.mac, parsed.mac);

            let framed = serialize_framed(&pkt, &cfg).expect("framed serialize");
            let parsed_framed = parse_framed(&framed, &cfg).expect("framed parse");
            assert_eq!(pkt.header, parsed_framed.header);
            assert_eq!(pkt.payload, parsed_framed.payload);
            assert_eq!(pkt.mac, parsed_framed.mac);
        }
    }

    fn handshake_accept_sample(cfg: &ProtocolConfig) -> Packet {
        let session_id = 0x11_22_33_44;
        Packet {
            header: PacketHeader {
                session_id,
                counter: 0,
                kind: PacketKind::Handshake,
                flags: PacketFlags {
                    encrypted: false,
                    needs_ack: true,
                    retransmit: false,
                },
            },
            payload: Payload::HandshakeAccept { session_id },
            mac: vec![0x44; cfg.security.mac_len],
        }
    }

    #[test]
    fn framed_parse_rejects_bad_lengths() {
        let cfg = demo_config();
        let pkt = sample_packets(&cfg).pop().unwrap();
        let mut framed = serialize_framed(&pkt, &cfg).expect("framed serialize");

        // Corrupt payload length to mismatch total bytes.
        framed[HEADER_LEN] = 0xFF;
        framed[HEADER_LEN + 1] = 0x00;
        assert!(matches!(
            parse_framed(&framed, &cfg),
            Err(ParseError::UnexpectedLength)
        ));
    }

    #[test]
    fn framed_parse_rejects_bad_mac_len() {
        let cfg = demo_config();
        let pkt = sample_packets(&cfg).pop().unwrap();
        let mut framed = serialize_framed(&pkt, &cfg).expect("framed serialize");

        // Drop one byte from the MAC.
        framed.pop();

        assert!(matches!(
            parse_framed(&framed, &cfg),
            Err(ParseError::MacLengthMismatch)
        ));
    }

    #[test]
    fn aead_rejects_header_tamper() {
        let cfg = demo_config();
        let pkt = sample_packets(&cfg).pop().unwrap();
        let nonce = [0x99; NONCE_BYTES];
        let dummy = DummyAead;

        let framed = seal_framed(&pkt, &cfg, &dummy, &nonce).expect("seal");
        let mut tampered = framed.clone();
        tampered[0] ^= 0xFF; // flip session_id byte

        assert!(matches!(
            open_framed(&tampered, &cfg, &dummy, &nonce),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn real_aead_roundtrip() {
        let cfg = demo_config();
        let pkt = sample_packets(&cfg).pop().unwrap();
        let key = [0xAB; KEY_BYTES];
        let nonce = [0x01; NONCE_BYTES];
        let aead = RealAead::new(key);

        let framed = seal_framed(&pkt, &cfg, &aead, &nonce).expect("seal");
        let parsed = open_framed(&framed, &cfg, &aead, &nonce).expect("open");
        assert_eq!(pkt.header, parsed.header);
        assert_eq!(pkt.payload, parsed.payload);

        // Tamper ciphertext to ensure auth fails.
        let mut bad = framed.clone();
        bad[HEADER_LEN + 2] ^= 0xFF;
        assert!(matches!(
            open_framed(&bad, &cfg, &aead, &nonce),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn real_aead_rejects_header_tamper() {
        let cfg = demo_config();
        let pkt = sample_packets(&cfg).pop().unwrap();
        let key = [0xCD; KEY_BYTES];
        let nonce = [0x02; NONCE_BYTES];
        let aead = RealAead::new(key);

        let framed = seal_framed(&pkt, &cfg, &aead, &nonce).expect("seal");
        let mut tampered = framed.clone();
        tampered[0] ^= 0xAA; // flip session_id byte

        assert!(matches!(
            open_framed(&tampered, &cfg, &aead, &nonce),
            Err(CryptoError::AuthFailed)
        ));
    }
}
