#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "std")]
use std::println;

#[cfg(not(feature = "std"))]
use alloc::vec;

use proto::{
    derive_nonce, seal_framed, validate_packet, DummyAead, Packet, PacketFlags, PacketHeader,
    PacketKind, Payload, SessionKeys, SESSION_SALT_BYTES,
};

#[cfg(feature = "std")]
fn main() {
    keyboard_task();
}

#[cfg(not(feature = "std"))]
fn main() {}

/// Simulates a warm-wake send path using cached session + deterministic nonce.
/// `embassy` feature uses no_std-friendly types; std build prints for inspection.
#[cfg_attr(feature = "embassy", allow(dead_code))]
pub fn keyboard_task() {
    let cfg = proto::demo_config();
    let session_id = 0xFE_ED_F0_0D;
    let mut session = SessionKeys::new(session_id, [0x99; SESSION_SALT_BYTES]);

    // Assume persisted counter from previous uptime.
    session.resume_from(7);
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
        mac: vec![0x55; cfg.security.mac_len],
    };

    // Validate locally before transmit (mirrors firmware-side sanity checks).
    validate_packet(&key_report, &cfg, Some(session_id), Some(counter - 1)).unwrap();

    let nonce = derive_nonce(&session.salt, counter);
    let aead = DummyAead;
    let frame = seal_framed(&key_report, &cfg, &aead, &nonce).unwrap();

    #[cfg(feature = "std")]
    println!(
        "firmware skeleton: seq={} len={} (dummy-aead)",
        counter,
        frame.len()
    );
}
