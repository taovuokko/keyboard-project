# Feather nRF52840 Express UF2 firmware

Minimal blinky firmware for Adafruit Feather nRF52840 Express with the TinyUF2 + SoftDevice bootloader. Uses `nrf52840-hal`, builds for `thumbv7em-none-eabihf`, and converts to UF2 via `elf2uf2-rs`.

## Prereqs
- `rustup target add thumbv7em-none-eabihf`
- `cargo install elf2uf2-rs`
- Put the board in UF2 mode (double-tap reset), mount `FTHR840BOOT`, and open `INFO_UF2.TXT`.

From `INFO_UF2.TXT`, note:
- `App start` (typical Feather: `0x26000`, but **use the file value**)
- `Flash size`
- `FamilyID` (`0xADA52840`)

## Build and flash
```sh
cd firmware/feather-fw
# tell build.rs the layout (from INFO_UF2.TXT)
APP_BASE=0x26000 FLASH_SIZE=0x100000 cargo build --release

# convert to UF2 (base must match APP_BASE)
elf2uf2-rs target/thumbv7em-none-eabihf/release/feather-fw --base 0x26000 --family 0xADA52840 feather-fw.uf2

# copy to the boot volume (adjust path to your mount point)
cp feather-fw.uf2 /run/media/$USER/FTHR840BOOT/
```

You can also set `UF2_INFO_PATH=/run/media/$USER/FTHR840BOOT/INFO_UF2.TXT` and omit `APP_BASE/FLASH_SIZE`; `build.rs` will auto-read the values.

## Notes
- LED pin: P1.15 (red user LED).
- RAM defaults to full 256 KiB; override with `RAM_BASE/RAM_SIZE` env vars if your SoftDevice config requires smaller RAM.
