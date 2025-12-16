use clap::Parser;
use proto::{
    associated_data, demo_config, derive_nonce, encode_header, encode_payload, sample_packets,
    seal_framed, sim::MockRf, simulate_wake_sequence, validate_packet, DummyAead, RealAead,
    SessionKeys, SimEvent, ValidationError, KEY_BYTES, MAX_RETRANSMIT_ATTEMPTS, SESSION_SALT_BYTES,
};
use serde::Serialize;

fn main() {
    let args = Args::parse();
    let cfg = demo_config();
    let frames = simulate_wake_sequence(&cfg);
    let packets = sample_packets(&cfg);
    let use_real_aead = args.real_aead;
    let aead_key = args.aead_key.unwrap_or([0x42; KEY_BYTES]);
    let demo_salt = args.session_salt.unwrap_or([0xA5; SESSION_SALT_BYTES]);
    let mut rf = if args.mock_rf {
        Some(MockRf::new(args.drop_first, args.reorder, args.jitter_ms))
    } else {
        None
    };
    let aead: Box<dyn proto::Aead> = if use_real_aead {
        Box::new(RealAead::new(aead_key))
    } else {
        Box::new(DummyAead)
    };

    println!("Host-side wake/auth simulation");
    println!(
        "crypto: {:?} mac_len={} bytes",
        cfg.security.cipher_suite, cfg.security.mac_len
    );
    println!("max payload: {} bytes", cfg.max_payload_bytes);
    println!(
        "retransmit policy: max {} retry, no backoff (latency-bounded)",
        MAX_RETRANSMIT_ATTEMPTS
    );
    println!(
        "latency target/max: {} ms / {} ms",
        cfg.latency.target.as_millis(),
        cfg.latency.max.as_millis()
    );
    println!();

    for frame in frames {
        match frame.event {
            SimEvent::KeyboardWakes => {
                println!("[{:>4} ms] keyboard wakes, opens listen window", frame.t_ms);
            }
            SimEvent::LinkAuthenticated => {
                println!("[{:>4} ms] link authenticated (Noise X25519)", frame.t_ms);
            }
            SimEvent::KeyReport { keys } => {
                println!("[{:>4} ms] key report sent {:?}", frame.t_ms, keys);
            }
            SimEvent::Idle => {
                println!("[{:>4} ms] returning to idle sleep", frame.t_ms);
            }
        }
    }

    println!("\nSample packet flow (header/payload/MAC expectations):");
    let mut last_counter = None;
    let session_id = 0x88_77_66_55;
    for pkt in &packets {
        let header = &pkt.header;
        print!(
            "- {:?} seq={} session=0x{:08x} flags={{enc:{}, ack:{}, rt:{}}}",
            header.kind,
            header.counter,
            header.session_id,
            header.flags.encrypted,
            header.flags.needs_ack,
            header.flags.retransmit,
        );

        let verdict = validate_packet(&pkt, &cfg, Some(header.session_id), last_counter);
        match verdict {
            Ok(()) => println!(" -> ok"),
            Err(ValidationError::PayloadTooLarge) => println!(" -> payload too large"),
            Err(ValidationError::MissingMac) => println!(" -> missing mac"),
            Err(ValidationError::ReplayDetected) => println!(" -> replay"),
            Err(ValidationError::CounterJump) => println!(" -> counter jump"),
            Err(ValidationError::SessionMismatch) => println!(" -> session mismatch"),
        }

        last_counter = Some(header.counter);
    }

    let mut delivered_frames = Vec::new();

    println!("\nByte layout preview (header / payload / AAD / framed):");
    for pkt in packets {
        let header_bytes = encode_header(&pkt.header);
        let payload_bytes = encode_payload(&pkt.payload);
        let aad = associated_data(&pkt.header, payload_bytes.len());
        let framed_len = header_bytes.len() + 2 + payload_bytes.len() + pkt.mac.len();
        println!(
            "- {:?}: header_len={} payload_len={} aad_len={} framed_len={}",
            pkt.header.kind,
            header_bytes.len(),
            payload_bytes.len(),
            aad.len(),
            framed_len,
        );
        let framed = proto::serialize_framed(&pkt, &cfg).expect("frame");
        let hex: String = framed.iter().map(|b| format!("{:02x}", b)).collect();
        println!("  hex (plaintext framing): {}", hex);

        let nonce = derive_nonce(&demo_salt, pkt.header.counter);
        let sealed = seal_framed(&pkt, &cfg, aead.as_ref(), &nonce).expect("seal");
        let sealed_hex: String = sealed.iter().map(|b| format!("{:02x}", b)).collect();
        println!(
            "  {} frame: {}",
            if use_real_aead {
                "xchacha20-poly1305"
            } else {
                "dummy-aead"
            },
            sealed_hex
        );

        if let Some(rf) = rf.as_mut() {
            rf.push(sealed);
            rf.advance(2);
            while let Some(delivered) = rf.pop() {
                println!("    mock-rf delivered frame ({} bytes)", delivered.len());
                delivered_frames.push(delivered);
            }
        }
    }

    if let Some(rf) = rf.as_ref() {
        let stats = rf.stats();
        println!(
            "\nmock-rf stats: delivered={} dropped={} last_time_ms={}",
            stats.delivered, stats.dropped, stats.last_time_ms
        );
        let metrics = Metrics {
            attempts: Some((delivered_frames.len() + stats.dropped) as u32),
            delivered: stats.delivered,
            dropped: stats.dropped,
            latency_ms: stats.last_time_ms,
            jitter_ms: args.jitter_ms,
            drop_first: args.drop_first,
            reorder: args.reorder,
            mock_rf_enabled: args.mock_rf,
            real_aead: use_real_aead,
        };
        println!(
            "\nmock-rf metrics json: {}",
            serde_json::to_string(&metrics).unwrap()
        );
        if let Some(path) = args.metrics_csv.as_ref() {
            let mut content = String::new();
            content.push_str("attempts,delivered,dropped,latency_ms,jitter_ms,drop_first,reorder,mock_rf,real_aead\n");
            content.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                metrics.attempts.unwrap_or(0),
                stats.delivered,
                stats.dropped,
                stats.last_time_ms,
                args.jitter_ms,
                args.drop_first,
                args.reorder,
                args.mock_rf,
                use_real_aead
            ));
            std::fs::write(path, content).expect("write metrics csv");
            println!("mock-rf metrics written to {}", path);
        }
    }

    println!("\nWarm wake (cached session, no handshake):");
    let mut warm_session = SessionKeys::new(session_id, [0xA5; SESSION_SALT_BYTES]);
    if let Some(resume) = args.resume_counter {
        warm_session.resume_from(resume);
    } else if let Some(next) = last_counter.map(|c| c + 1) {
        warm_session.resume_from(next);
    }
    let warm_counter = warm_session.next_counter().expect("counter not exhausted");
    let nonce = warm_session.nonce_for(warm_counter);
    println!(
        "- next seq={} uses deterministic nonce (salt || counter) prefix={:02x?}",
        warm_counter,
        &nonce[..SESSION_SALT_BYTES]
    );

    if let Some(rf) = rf {
        let stats = rf.stats();
        let metrics = Metrics {
            attempts: None, // set later if we add retry loop; here we just deliver per frame
            delivered: stats.delivered,
            dropped: stats.dropped,
            latency_ms: stats.last_time_ms,
            jitter_ms: args.jitter_ms,
            drop_first: args.drop_first,
            reorder: args.reorder,
            mock_rf_enabled: args.mock_rf,
            real_aead: use_real_aead,
        };
        println!(
            "\nmock-rf metrics json: {}",
            serde_json::to_string(&metrics).unwrap()
        );
        if let Some(path) = args.metrics_csv.as_ref() {
            let mut content = String::new();
            content.push_str(
                "delivered,dropped,latency_ms,jitter_ms,drop_first,reorder,mock_rf,real_aead\n",
            );
            content.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                stats.delivered,
                stats.dropped,
                stats.last_time_ms,
                args.jitter_ms,
                args.drop_first,
                args.reorder,
                args.mock_rf,
                use_real_aead
            ));
            std::fs::write(path, content).expect("write metrics csv");
            println!("mock-rf metrics written to {}", path);
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// Use real XChaCha20-Poly1305 AEAD instead of dummy tagger.
    #[arg(long, default_value_t = false)]
    real_aead: bool,

    /// 32-byte AEAD key as hex (64 chars). Defaults to 0x42 * 32.
    #[arg(long, value_parser = parse_key)]
    aead_key: Option<[u8; KEY_BYTES]>,

    /// 16-byte session salt as hex (32 chars). Defaults to 0xA5 * 16.
    #[arg(long, value_parser = parse_salt)]
    session_salt: Option<[u8; SESSION_SALT_BYTES]>,

    /// Enable mock RF: drop first frame, reorder, and add jitter before "delivery".
    #[arg(long, default_value_t = false)]
    mock_rf: bool,

    /// Drop the first frame in mock RF.
    #[arg(long, default_value_t = true)]
    drop_first: bool,

    /// Reorder frames in mock RF.
    #[arg(long, default_value_t = true)]
    reorder: bool,

    /// Jitter in ms for mock RF.
    #[arg(long, default_value_t = 2)]
    jitter_ms: u64,

    /// Persisted session counter for warm wake (default: next after last seen).
    #[arg(long)]
    resume_counter: Option<u32>,

    /// Path to write mock RF metrics CSV (optional).
    #[arg(long)]
    metrics_csv: Option<String>,
}

#[derive(Serialize)]
struct Metrics {
    attempts: Option<u32>,
    delivered: usize,
    dropped: usize,
    latency_ms: u64,
    jitter_ms: u64,
    drop_first: bool,
    reorder: bool,
    mock_rf_enabled: bool,
    real_aead: bool,
}

fn parse_key(s: &str) -> Result<[u8; KEY_BYTES], String> {
    let bytes = parse_hex_bytes(s, KEY_BYTES)?;
    Ok(bytes.try_into().unwrap())
}

fn parse_salt(s: &str) -> Result<[u8; SESSION_SALT_BYTES], String> {
    let bytes = parse_hex_bytes(s, SESSION_SALT_BYTES)?;
    Ok(bytes.try_into().unwrap())
}

fn parse_hex_bytes(s: &str, expected: usize) -> Result<Vec<u8>, String> {
    if s.len() != expected * 2 {
        return Err(format!(
            "expected {} hex chars ({} bytes), got {}",
            expected * 2,
            expected,
            s.len()
        ));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
