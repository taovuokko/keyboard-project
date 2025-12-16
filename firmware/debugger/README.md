# debugger-fw (STM32F746G-DISCO)

General-purpose hardware debugger firmware for the radio path. Uses the STM32F746G-DISCO board you already have, so USB on the nRF side can stay out of the loop. Grow this alongside new protocol features to probe GPIOs, timestamps, and serial logs.

## Build
```sh
cd firmware/debugger
cargo build --release
```

## Flash
- `openocd -f board/stm32f7discovery.cfg -c "program target/thumbv7em-none-eabihf/release/debugger-fw verify reset exit"`  
  (or use `st-flash write ...` if you prefer the ST tools)

## Observe RTT logs
- Attach with your debugger (e.g. `probe-rs rtt --chip STM32F746NGH6`) and you should see `boot` on every reset/power-on/flash.
- `HardFault` dumps CFSR/HFSR/MMFAR/BFAR plus PC/LR over RTT; the handler parks in a BKPT loop so you can attach and inspect state.

## Notes
- `memory.x` targets the on-board 1 MiB flash and the full 320 KiB RAM window (DTCM + SRAM1 + SRAM2) starting at `0x20000000`.
- `src/main.rs` currently just idles (`wfi`) after taking peripherals—safe starting point for adding GPIO probes, timers, UART/SWO logging, or RF front-end sniffing without touching USB.
