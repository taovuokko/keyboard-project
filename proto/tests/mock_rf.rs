use proto::{
    derive_nonce, open_framed, seal_framed, sim::MockRf, validate_packet, DummyAead, Packet,
    PacketFlags, PacketHeader, PacketKind, Payload, SessionKeys, MAX_RETRANSMIT_ATTEMPTS,
    SESSION_SALT_BYTES,
};

#[test]
fn mock_rf_with_drop_and_reorder_delivers_ack_after_retransmit() {
    let cfg = proto::demo_config();
    let session_id = 0x77_88_99_AA;
    let mut session = SessionKeys::new(session_id, [0xCC; SESSION_SALT_BYTES]);
    let aead = DummyAead;
    let mut rf = MockRf::new(true, true, 2); // drop first send, reorder, 2ms jitter

    // Simulate a single key report with retransmit allowance.
    let counter = session.next_counter().expect("counter not exhausted");
    let payload = Payload::KeyReport { keys: vec![0x04] };
    let pkt = Packet {
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
        payload,
        mac: vec![0xAA; cfg.security.mac_len],
    };

    let mut acked = false;
    let mut attempts = 0;
    let mut total_latency_ms = 0;
    for attempt in 0..=MAX_RETRANSMIT_ATTEMPTS {
        attempts = attempt + 1;
        let nonce = session.nonce_for(counter);
        let frame = seal_framed(&pkt, &cfg, &aead, &nonce).expect("seal");
        rf.push(frame);
        rf.advance(2); // allow frames to become deliverable

        // Process anything in flight (data or reordered frames).
        while let Some(rx_frame) = rf.pop() {
            // Try parsing as our packet kind; if it fails, ignore.
            if let Ok(parsed) = open_framed(&rx_frame, &cfg, &aead, &nonce) {
                // Receiver validation
                validate_packet(&parsed, &cfg, Some(session_id), Some(counter - 1))
                    .expect("receiver validate");

                // Build ack
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
                let ack_frame = seal_framed(&ack, &cfg, &aead, &ack_nonce).expect("ack seal");

                // To force reorder, also inject a keepalive after the ack.
                rf.push(ack_frame);
                let keepalive = Packet {
                    header: PacketHeader {
                        session_id,
                        counter: ack.header.counter + 1,
                        kind: PacketKind::KeepAlive,
                        flags: PacketFlags {
                            encrypted: true,
                            needs_ack: false,
                            retransmit: false,
                        },
                    },
                    payload: Payload::KeepAlive,
                    mac: vec![0xCC; cfg.security.mac_len],
                };
                let keep_nonce = session.nonce_for(keepalive.header.counter);
                let keep_frame =
                    seal_framed(&keepalive, &cfg, &aead, &keep_nonce).expect("keep seal");
                rf.push(keep_frame);
                rf.advance(2);
            } else {
                // Try parse as ack (using next nonce).
                let ack_nonce = derive_nonce(&session.salt, counter + 1);
                if let Ok(parsed_ack) = open_framed(&rx_frame, &cfg, &aead, &ack_nonce) {
                    if matches!(parsed_ack.payload, Payload::Ack { ack_counter } if ack_counter == counter)
                    {
                        acked = true;
                        break;
                    }
                }
            }
        }

        if acked {
            total_latency_ms = rf.stats().last_time_ms;
            break;
        }

        if attempt == MAX_RETRANSMIT_ATTEMPTS {
            panic!("failed to deliver after retransmit");
        }
    }

    assert!(acked, "ack not received after retransmit and reorder");
    let stats = rf.stats();
    println!(
        "mock rf stats: attempts={} delivered={} dropped={} latency_ms={}",
        attempts, stats.delivered, stats.dropped, total_latency_ms
    );
}

#[test]
fn mock_rf_drop_ack_stops_after_retry() {
    let cfg = proto::demo_config();
    let session_id = 0x01_23_45_67;
    let mut session = SessionKeys::new(session_id, [0xDD; SESSION_SALT_BYTES]);
    let aead = DummyAead;
    let mut rf = MockRf::new(false, false, 0);

    // Data packet.
    let counter = session.next_counter().expect("counter not exhausted");
    let pkt = Packet {
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

    let mut attempts = 0;
    let mut acked = false;
    while attempts <= MAX_RETRANSMIT_ATTEMPTS {
        attempts += 1;
        let nonce = session.nonce_for(counter);
        let frame = seal_framed(&pkt, &cfg, &aead, &nonce).expect("seal");
        rf.push(frame);

        // process data frame
        while let Some(rx_frame) = rf.pop() {
            if let Ok(parsed) = open_framed(&rx_frame, &cfg, &aead, &nonce) {
                validate_packet(&parsed, &cfg, Some(session_id), Some(counter - 1))
                    .expect("receiver validate");
                // Drop the ACK deliberately on first attempt.
                if attempts == 1 {
                    // simulate drop by not pushing ack
                } else {
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
                    let ack_frame = seal_framed(&ack, &cfg, &aead, &ack_nonce).expect("ack seal");
                    rf.push(ack_frame);
                }
            } else {
                let ack_nonce = derive_nonce(&session.salt, counter + 1);
                if let Ok(parsed_ack) = open_framed(&rx_frame, &cfg, &aead, &ack_nonce) {
                    if matches!(parsed_ack.payload, Payload::Ack { ack_counter } if ack_counter == counter)
                    {
                        acked = true;
                    }
                }
            }
        }

        if acked {
            break;
        }
    }

    assert!(
        attempts == MAX_RETRANSMIT_ATTEMPTS + 1,
        "should stop after max retries"
    );
    assert!(acked, "ack not received by final retry");
}
