# Protocol Notes (Draft)

This document captures the evolving wire-format and cryptography choices for the wireless keyboard system. It is simulation-first and will be refined before firmware implementation.

## Crypto
- Cipher suite: `XChaCha20-Poly1305`
- Nonce: 24 bytes (XChaCha); final derivation will be `session_salt || packet_counter` (concatenation), no per-packet RNG needed.
- MAC/tag length: 16 bytes (auth tag)
- Key material: 32-byte keys
- Handshake: Noise X25519 (cold start or when no valid cached session exists) or pre-shared mode for provisioning; forward-secure rekeying expected per session. Warm wake uses cached session keys to skip the handshake and hit instant wake goals.

## Packet Header
- `session_id` (u32)
- `counter` (u32, strictly increasing; replay and jump checks applied; counters are scoped per session and reset on session reset)
- `kind` enum: Handshake, Control, KeyReport, Ack, KeepAlive
- `flags`: `encrypted`, `needs_ack`, `retransmit`

## Payloads
- `HandshakeInit`: 32-byte ephemeral public key + 24-byte nonce
- `HandshakeAccept`: u32 session_id
- `Control`: code (u8) + data (vec)
- `KeyReport`: key bytes (vec)
- `Ack`: ack_counter (u32)
- `KeepAlive`: empty

## Validation Rules (current sim)
- MAC/tag must be present and match configured length.
- Payload length must not exceed `max_payload_bytes` (workspace default: 32 bytes).
- Counters must not repeat; backward or large jumps are rejected.
- Session ID must match expected once established.

## Timing Anchors
- Wake listen window: 8 ms (default)
- Reconnect timeout: 2 ms budget for host reply
- Latency target/max: 6 ms / 10 ms end-to-end budget

## Session Rekey / Forward Secrecy
- Ephemeral Noise X25519 handshake is executed once per session (cold start or cache miss), not on every wake. Warm wake reuses the cached session to hit instant wake.
- Rekey trigger: new session on device reboot or explicit re-pair; add host-driven rekey control code later if needed.
- Session state (session_id, salt, next counter) must be persisted for warm wake; rotate session on cache invalidation to preserve forward secrecy.


## Payload Size Rationale
- `max_payload_bytes = 32` to minimize airtime and keep frames within a single 2.4 GHz RF packet on common transceivers while carrying a full key report plus headers.***
