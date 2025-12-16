use proto::{
    open_framed, seal_framed, validate_packet, DummyAead, Packet, PacketFlags, PacketHeader,
    PacketKind, Payload, SessionKeys, SESSION_SALT_BYTES,
};

#[test]
fn warm_wake_uses_cached_session() {
    let cfg = proto::demo_config();
    let session_id = 0xDE_AD_BE_EF;
    let mut session = SessionKeys::new(session_id, [0x55; SESSION_SALT_BYTES]);

    // Simulate persisted counter after previous session (no handshake on warm wake).
    session.resume_from(5);
    let counter = session.next_counter().expect("counter not exhausted"); // should be 5
    assert_eq!(counter, 5);

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
        mac: vec![0xAA; cfg.security.mac_len],
    };

    // Last seen counter was 4 in the cached session; ensure validation accepts cached session.
    validate_packet(&key_report, &cfg, Some(session_id), Some(counter - 1))
        .expect("warm wake validate");

    let nonce = session.nonce_for(counter);
    let aead = DummyAead;
    let sealed = seal_framed(&key_report, &cfg, &aead, &nonce).expect("seal");
    let opened = open_framed(&sealed, &cfg, &aead, &nonce).expect("open");
    assert_eq!(key_report.header, opened.header);
    assert_eq!(key_report.payload, opened.payload);
}
