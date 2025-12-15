# Energy Budget Notes (Draft)

Goal: sanity-check cold vs warm wake costs and radio airtime to guide MCU selection and power targets. Numbers below use rough, defensible assumptions; replace with measured values on target silicon.

## Assumptions
- Supply: 3.0 V
- MCU active current (crypto): ~10 mA @ 64 MHz (Cortex-M4 class)
- Radio TX/RX: 15 mA TX, 13 mA RX @ 1 Mbps effective
- X25519 scalar mult time: ~2 ms on Cortex-M4
- Packet sizes: handshake ~84 bytes framed; key report ~30 bytes framed; ack ~32 bytes framed
- Airtime @ 1 Mbps: `time = bytes * 8 / 1e6`

## Cold Wake (with handshake)
- Crypto: X25519 ≈ 2 ms → Energy ≈ 10 mA * 3.0 V * 0.002 s ≈ **60 µJ**
- Handshake TX airtime (84 B): 84 * 8 / 1e6 ≈ 0.67 ms → Energy ≈ 15 mA * 3.0 V * 0.00067 s ≈ **30 µJ**
- Handshake RX airtime similar scale; total radio ~50–60 µJ
- Cold wake budget ≈ **110–120 µJ** (crypto + radio), excluding app work

## Warm Wake (cached session)
- Key report TX airtime (≈30 B): 30 * 8 / 1e6 ≈ 0.24 ms → Energy ≈ 15 mA * 3.0 V * 0.00024 s ≈ **11 µJ**
- Ack RX airtime (≈32 B): 32 * 8 / 1e6 ≈ 0.26 ms → Energy ≈ 13 mA * 3.0 V * 0.00026 s ≈ **10 µJ**
- Minimal CPU (no crypto): ~0.5 ms @ 5 mA → **7.5 µJ**
- Warm wake budget ≈ **30 µJ**

## Retransmit Impact
- Single retry (data + ack) roughly doubles radio portion: +~20 µJ warm wake, +~50 µJ cold wake (handshake generally not retried).

## Design Implications
- Cached sessions dramatically reduce wake energy (≈4x lower than cold).
- Keeping handshake to cold-start/pair/rekey only is important for battery life and instant wake UX.
- MCU: any Cortex-M4F/M33 that can do X25519 in ~2 ms at ~10 mA active meets these budgets; hardware accel can lower cold-wake cost further.
- Radio airtime dominates warm wake; keep frames ≤32 B and retries minimal to stay within ~30–50 µJ per keypress.***
