use proto::{
    open_framed, seal_framed, sim::MockRf, validate_packet, DummyAead, Packet, PacketFlags,
    PacketHeader, PacketKind, Payload, SessionKeys, SESSION_SALT_BYTES,
};

#[test]
fn warm_wake_survives_drop_and_reorder() {
    let cfg = proto::demo_config();
    let session_id = 0xAA_BB_CC_DD;
    let mut session = SessionKeys::new(session_id, [0x11; SESSION_SALT_BYTES]);
    session.resume_from(10);
    let aead = DummyAead;
    let mut rf = MockRf::new(true, true, 1); // drop first, reorder, jitter 1ms

    // Warm-wake data packet (no handshake).
    let counter = session.next_counter().expect("counter not exhausted"); // starts at 10
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

    let mut acked = false;
    let mut _attempts = 0;
    let nonce = session.nonce_for(counter);
    let frame = seal_framed(&key_report, &cfg, &aead, &nonce).expect("seal");
    rf.push(frame);

    let mut retries = 0;
    while retries <= proto::MAX_RETRANSMIT_ATTEMPTS.into() {
        _attempts += 1;
        // process frames with reorder/drop
        while let Some(rx) = rf.pop() {
            if let Ok(parsed) = open_framed(&rx, &cfg, &aead, &nonce) {
                validate_packet(&parsed, &cfg, Some(session_id), Some(counter - 1))
                    .expect("validate warm");

                // send ack
                let ack = Packet {
                    header: PacketHeader {
                        session_id,
                        counter: counter + 1,
                        kind: PacketKind::Ack,
                        flags: PacketFlags {
                            encrypted: true,
                            needs_ack: false,
                            retransmit: false,
                        },
                    },
                    payload: Payload::Ack {
                        ack_counter: parsed.header.counter,
                    },
                    mac: vec![0xBB; cfg.security.mac_len],
                };
                let ack_nonce = session.nonce_for(ack.header.counter);
                let ack_frame = seal_framed(&ack, &cfg, &aead, &ack_nonce).expect("ack");
                rf.push(ack_frame);
                rf.advance(1);
            } else {
                let ack_nonce = session.nonce_for(counter + 1);
                if let Ok(parsed_ack) = open_framed(&rx, &cfg, &aead, &ack_nonce) {
                    if matches!(parsed_ack.payload, Payload::Ack { ack_counter } if ack_counter == counter)
                    {
                        acked = true;
                        break;
                    }
                }
            }
        }

        if acked {
            break;
        }

        if retries < proto::MAX_RETRANSMIT_ATTEMPTS.into() {
            retries += 1;
            // resend data (warm-wake retransmit)
            let retry_nonce = session.nonce_for(counter);
            let frame = seal_framed(&key_report, &cfg, &aead, &retry_nonce).expect("retry seal");
            rf.push(frame);
            rf.advance(1);
        } else {
            break;
        }
    }

    assert!(
        acked,
        "warm wake packet should be acked even with drop/reorder"
    );
}
