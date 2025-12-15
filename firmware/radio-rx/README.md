# radio-rx (nice!nano / nRF52840)

Listens for 2 Mbps proprietary packets on channel 7 (2.407 GHz), address 0xE7E7E7E7E7. On a good packet, LED P0.13 blips for 50 ms. Idle heartbeat: short blip once per second.

Use together with `firmware/radio-tx` (same channel/address, 8-byte payload `PINGPONG` with rolling counter).

## Build
```sh
cd firmware/radio-rx
APP_BASE=0x26000 FLASH_SIZE=0x100000 cargo build --release
```

Convert to UF2 as with `feather-fw` (base 0x26000, family 0xADA52840).

Flash: double-tap reset → copy UF2 → single reset to app. No UF2 drive when running app.***
