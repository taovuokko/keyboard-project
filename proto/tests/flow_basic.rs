use proto::{
    open_framed, parse_framed, seal_framed, serialize_framed, validate_packet, CipherSuite,
    DummyAead, HandshakeKind, Packet, PacketFlags, PacketHeader, PacketKind, Payload,
    ProtocolConfig, SessionKeys, SESSION_SALT_BYTES,
};

#[test]
fn cold_handshake_then_keyreport_flow() {
    let cfg = ProtocolConfig {
        wake: proto::WakeTiming {
            idle_sleep: std::time::Duration::from_millis(200),
            listen_window: std::time::Duration::from_millis(8),
            reconnect_timeout: std::time::Duration::from_millis(2),
        },
        security: proto::SecurityConfig {
            handshake: HandshakeKind::NoiseX25519,
            forward_secure: true,
            replay_protection: true,
            cipher_suite: CipherSuite::XChaCha20Poly1305,
            mac_len: proto::MAX_MAC_BYTES,
        },
        latency: proto::LatencyBudget {
            target: std::time::Duration::from_millis(6),
            max: std::time::Duration::from_millis(10),
        },
        max_payload_bytes: 32,
    };

    let session_id = 0xCA_FE_BA_BE;
    let mut session = SessionKeys::new(session_id, [0x11; SESSION_SALT_BYTES]);

    // Handshake init (counter 0)
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
            eph_pubkey: [0xAA; proto::KEY_BYTES],
            nonce: session.handshake_nonce(),
        },
        mac: vec![0x01; cfg.security.mac_len],
    };

    validate_packet(&handshake, &cfg, None, None).expect("handshake validate");
    let hs_framed = serialize_framed(&handshake, &cfg).expect("handshake frame");
    let hs_parsed = parse_framed(&hs_framed, &cfg).expect("handshake parse");
    assert_eq!(handshake.header, hs_parsed.header);
    assert_eq!(handshake.payload, hs_parsed.payload);

    // Handshake accept (counter 0, still session-scoped)
    let handshake_accept = Packet {
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
        mac: vec![0x02; cfg.security.mac_len],
    };

    validate_packet(&handshake_accept, &cfg, Some(session_id), None)
        .expect("handshake accept validate");

    // Data packet (counter > handshake)
    let counter = session.next_counter().expect("counter not exhausted");
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
        mac: vec![0x03; cfg.security.mac_len],
    };

    validate_packet(
        &key_report,
        &cfg,
        Some(session_id),
        Some(handshake_accept.header.counter),
    )
    .expect("key report validate");
    let kr_framed = serialize_framed(&key_report, &cfg).expect("key report frame");
    let kr_parsed = parse_framed(&kr_framed, &cfg).expect("key report parse");
    assert_eq!(key_report.header, kr_parsed.header);
    assert_eq!(key_report.payload, kr_parsed.payload);

    // AEAD flow using dummy implementation (no real crypto, but MAC checked)
    let dummy = DummyAead;
    let nonce = session.nonce_for(counter);
    let sealed = seal_framed(&key_report, &cfg, &dummy, &nonce).expect("aead seal");
    let opened = open_framed(&sealed, &cfg, &dummy, &nonce).expect("aead open");
    assert_eq!(key_report.header, opened.header);
    assert_eq!(key_report.payload, opened.payload);
}
