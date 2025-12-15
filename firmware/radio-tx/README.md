# radio-tx (nice!nano / nRF52840)

Minimal 2 Mbps radio beacon (proprietary NRFLite/ESB-style): sends 8-byte `PINGPONG` frames on channel 7 (2.407 GHz) with address 0xE7E7E7E7E7. LED P0.13 blinks to show loop alive.

## Build
```sh
cd firmware/radio-tx
APP_BASE=0x26000 FLASH_SIZE=0x100000 cargo build --release
```

## UF2
```sh
llvm-objcopy -O binary target/thumbv7em-none-eabihf/release/radio-tx target/thumbv7em-none-eabihf/release/radio-tx.bin
python ../feather-fw/../scripts/make-uf2.py   # not provided; use your existing UF2 helper or elf2uf2 tool with --base 0x26000
```

For now, reuse the python UF2 helper used for feather-fw (base 0x26000, family 0xADA52840).

## Flash
Double-tap reset → copy UF2 to `FTHR840BOOT`, single reset to app (no UF2 drive). LED P0.13 should blink.
