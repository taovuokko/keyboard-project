# led-test (nice!nano / nRF52840)

Minimal LED blink (P0.13) to verify app boots. No USB. Blink: 300 ms on / 300 ms off.

## Build
```sh
cd firmware/led-test
APP_BASE=0x26000 FLASH_SIZE=0x100000 cargo build --release
```

Convert to UF2 (base 0x26000, family 0xADA52840) using the same python helper used elsewhere or your UF2 tool. Example (python chunker used in this repo):
```sh
llvm-objcopy -O binary target/thumbv7em-none-eabihf/release/led-test target/thumbv7em-none-eabihf/release/led-test.bin
python - <<'PY'
import struct, math
from pathlib import Path
base = 0x26000
family = 0xADA52840
max_payload = 476
chunk = 256
bin_path = Path('target/thumbv7em-none-eabihf/release/led-test.bin')
out_path = bin_path.with_suffix('.uf2')
data = bin_path.read_bytes()
blocks = math.ceil(len(data) / chunk)
MAGIC_START0 = 0x0A324655
MAGIC_START1 = 0x9E5D5157
MAGIC_END = 0x0AB16F30
FLAG_FAMILY = 0x00002000
with out_path.open('wb') as f:
    for i in range(blocks):
        payload = data[i*chunk:(i+1)*chunk]
        payload_len = len(payload)
        payload = payload.ljust(max_payload, b'\\x00')
        header = struct.pack('<IIIIIIII', MAGIC_START0, MAGIC_START1, FLAG_FAMILY,
                             base + i*chunk, payload_len, i, blocks, family)
        f.write(header)
        f.write(payload)
        f.write(struct.pack('<I', MAGIC_END))
print(f'wrote {out_path}")
PY
```

Flash: double-tap reset → copy led-test.uf2 → single reset to app. UF2 drive disappears, LED blinks. If LED doesn't blink and lsusb shows only bootloader (239a:00b3), suspect hardware/boot issues.***
