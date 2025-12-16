#![cfg(feature = "proptest")]

use proptest::prelude::*;
use proto::{
    decode_header, decode_payload, encode_header, encode_payload, parse_packet, serialize_packet,
    Packet, PacketFlags, PacketHeader, PacketKind, Payload, ProtocolConfig, KEY_BYTES,
    MAX_MAC_BYTES, NONCE_BYTES,
};

fn cfg() -> ProtocolConfig {
    proto::demo_config()
}

fn arb_header() -> impl Strategy<Value = PacketHeader> {
    (
        any::<u32>(),
        any::<u32>(),
        prop_oneof![
            Just(PacketKind::Handshake),
            Just(PacketKind::Control),
            Just(PacketKind::KeyReport),
            Just(PacketKind::Ack),
            Just(PacketKind::KeepAlive),
        ],
        any::<(bool, bool, bool)>(),
    )
        .prop_map(|(session_id, counter, kind, (enc, ack, rt))| PacketHeader {
            session_id,
            counter,
            kind,
            flags: PacketFlags {
                encrypted: enc,
                needs_ack: ack,
                retransmit: rt,
            },
        })
}

fn arb_payload() -> impl Strategy<Value = Payload> {
    prop_oneof![
        Just(Payload::HandshakeAccept { session_id: 0 }),
        Just(Payload::KeepAlive),
        prop::collection::vec(any::<u8>(), 0..32).prop_map(|keys| Payload::KeyReport { keys }),
        prop::collection::vec(any::<u8>(), 1..16).prop_map(|data| Payload::Control {
            code: data[0],
            data: data.into_iter().skip(1).collect()
        }),
        Just(Payload::Ack { ack_counter: 1 }),
        Just(Payload::HandshakeInit {
            eph_pubkey: [0u8; KEY_BYTES],
            nonce: [0u8; NONCE_BYTES],
        }),
    ]
}

proptest! {
    #[test]
    fn header_roundtrip(h in arb_header()) {
        let bytes = encode_header(&h);
        let parsed = decode_header(&bytes).unwrap();
        prop_assert_eq!(h, parsed);
    }

    #[test]
    fn payload_roundtrip(h in arb_header(), p in arb_payload()) {
        let bytes = encode_payload(&p);
        let parsed = decode_payload(h.kind, &bytes);
    if matches_valid_kind_payload(h.kind, &p) {
        prop_assert!(parsed.is_ok());
    }
}

    #[test]
    fn serialize_parse_roundtrip(h in arb_header(), p in arb_payload()) {
        let cfg = cfg();
        let mac = vec![0xAA; MAX_MAC_BYTES];
        let pkt = Packet { header: h, payload: p, mac };
        // Ensure payload matches kind; otherwise expect parse to fail.
        let ser = serialize_packet(&pkt, &cfg);
        if let Ok(serialized) = ser {
            if matches_valid_kind_payload(pkt.header.kind, &pkt.payload) {
                let parsed = parse_packet(&serialized.header, &serialized.payload, &serialized.mac, &cfg).unwrap();
                prop_assert_eq!(pkt.header, parsed.header);
                prop_assert_eq!(pkt.payload, parsed.payload);
            }
        }
    }
}

fn matches_valid_kind_payload(kind: PacketKind, p: &Payload) -> bool {
    matches!(
        (kind, p),
        (PacketKind::Handshake, Payload::HandshakeInit { .. })
            | (PacketKind::Handshake, Payload::HandshakeAccept { .. })
            | (PacketKind::Control, Payload::Control { .. })
            | (PacketKind::KeyReport, Payload::KeyReport { .. })
            | (PacketKind::Ack, Payload::Ack { .. })
            | (PacketKind::KeepAlive, Payload::KeepAlive)
    )
}
