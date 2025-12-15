# Keyboard Project 

 For the wireless mechanical keyboard (sofle v2) The focus now is on protocol design and host-side validation before firmware hits hardware.

## Layout
- `proto/` shared protocol types (wake timing, security, latency budgets).
- `host-sim/` host-side simulation binary for wake/auth/key delivery timelines.
- `firmware/keyboard/` placeholder for low-power keyboard firmware.
- `firmware/dongle/` placeholder for USB dongle firmware.
- `docs/` design notes and protocol RFCs.

## Quickstart
```sh
cargo run -p host-sim    # prints a demo wake/auth/key timeline using proto defaults
cargo check              # validate workspace builds
```
Use Rust 1.82+ (matches other tooling in this repo).

### Features and modes
- `proto`: `std` (default), `crypto` (default, XChaCha20-Poly1305), `alloc` (no_std builds), `proptest` (property tests).
- `host-sim` flags: `--real-aead`, `--aead-key <64 hex>`, `--session-salt <32 hex>`, `--mock-rf` with `--drop-first/--reorder/--jitter-ms`, `--resume-counter <u32>`.
- `keyboard-skeleton`: `std` (default); `no_std` path available for embedding (uses alloc only).

### Checks
```sh
cargo test                                    # std + crypto
cargo test -p proto --features proptest       # property tests
cargo check -p proto --no-default-features --features alloc  # no_std+alloc path
cargo test --all-features                     # everything enabled
```
to be continue....