#![no_std]
#![no_main]

use cortex_m::asm;
use cortex_m_rt::{entry, exception, ExceptionFrame};
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};
use stm32f7::stm32f7x6;

const STABLE_SAMPLES: u8 = 3;
const LATCH_AUTO_CLEAR_MS: u32 = 5_000;

extern "C" {
    fn HAL_Init() -> i32;
    fn HAL_IncTick();
    fn HAL_SYSTICK_IRQHandler();
    fn HAL_GetTick() -> u32;
    fn board_clock_init();
    fn display_init();
    fn display_update(rx_ok: bool, tx_ok: bool, err_code: u32);
}

fn setup_rtt() {
    // Define the RTT control block once to avoid duplicate `_SEGGER_RTT` symbols.
    rtt_init_print!();
}

fn interpret_err(rx_ok: bool, tx_ok: bool) -> u32 {
    match (rx_ok, tx_ok) {
        (true, true) => 0x00,
        (false, true) => 0x01,
        (true, false) => 0x02,
        (false, false) => 0x03,
    }
}

fn err_reason(code: u32) -> &'static str {
    match code {
        0x01 => "RX low",
        0x02 => "TX low",
        0x03 => "RX+TX low",
        _ => "ok",
    }
}

#[entry]
fn main() -> ! {
    let dp = stm32f7x6::Peripherals::take().expect("stm32 peripherals already taken");
    let rcc = dp.RCC;
    let gpiob = dp.GPIOB;
    let gpiog = dp.GPIOG;
    let gpioi = dp.GPIOI;

    unsafe {
        HAL_Init();
        board_clock_init();
        display_init();
    }

    // Enable GPIOB (D3) and GPIOG (D2) for status inputs.
    rcc.ahb1enr.modify(|_, w| {
        w.gpioben().enabled();
        w.gpioien().enabled();
        w.gpiogen().enabled()
    });
    let _ = rcc.ahb1enr.read(); // delay after clock enable

    // D3 -> PB4, D2 -> PG6 as inputs with pulldowns so "FAIL" is the default.
    gpiob.moder.modify(|r, w| unsafe {
        w.bits(r.bits() & !(0b11 << (4 * 2))) // PB4 input
    });
    gpiob.pupdr.modify(|r, w| unsafe {
        w.bits((r.bits() & !(0b11 << (4 * 2))) | (0b10 << (4 * 2)))
    });

    gpiog.moder.modify(|r, w| unsafe {
        w.bits(r.bits() & !(0b11 << (6 * 2))) // PG6 input
    });
    gpiog.pupdr.modify(|r, w| unsafe {
        w.bits((r.bits() & !(0b11 << (6 * 2))) | (0b10 << (6 * 2)))
    });

    // D4 -> PG7 used as latch clear input with pull-down (active high).
    gpiog.moder.modify(|r, w| unsafe {
        w.bits(r.bits() & !(0b11 << (7 * 2))) // PG7 input
    });
    gpiog.pupdr.modify(|r, w| unsafe {
        w.bits((r.bits() & !(0b11 << (7 * 2))) | (0b10 << (7 * 2)))
    });

    // User button (PI11) as alternative latch clear, pull-down.
    gpioi.moder.modify(|r, w| unsafe {
        w.bits(r.bits() & !(0b11 << (11 * 2))) // PI11 input
    });
    gpioi.pupdr.modify(|r, w| unsafe {
        w.bits((r.bits() & !(0b11 << (11 * 2))) | (0b10 << (11 * 2)))
    });

    setup_rtt();
    rprintln!("boot");

    let mut tick_count: u32 = 0;

    let read_status = || {
        let rx_ok = gpiog.idr.read().idr6().bit_is_set();
        let tx_ok = gpiob.idr.read().idr4().bit_is_set();

        (rx_ok, tx_ok, interpret_err(rx_ok, tx_ok))
    };

    let (rx_ok, tx_ok, err) = read_status();
    unsafe {
        display_update(rx_ok, tx_ok, err);
    }

    let mut last_rx = rx_ok;
    let mut last_tx = tx_ok;
    let mut last_err: u32 = err;
    let mut shown_rx = rx_ok;
    let mut shown_tx = tx_ok;
    let mut shown_err = err;
    // Track raw samples for debounce.
    let mut rx_sample = rx_ok;
    let mut tx_sample = tx_ok;
    let mut rx_sample_count: u8 = STABLE_SAMPLES;
    let mut tx_sample_count: u8 = STABLE_SAMPLES;

    let mut sticky_err: Option<(u32, u32)> = if err != 0 { Some((err, 0)) } else { None };

    loop {
        if tick_count % 200 == 0 {
            rprintln!("tick");
        }
        tick_count = tick_count.wrapping_add(1);

        let now_ms = unsafe { HAL_GetTick() };
        let (rx_ok, tx_ok, _) = read_status();

        // Debounce: require 3 consecutive identical samples before accepting a change.
        if rx_ok == rx_sample {
            if rx_sample_count < STABLE_SAMPLES {
                rx_sample_count += 1;
            }
        } else {
            rx_sample = rx_ok;
            rx_sample_count = 1;
        }

        if tx_ok == tx_sample {
            if tx_sample_count < STABLE_SAMPLES {
                tx_sample_count += 1;
            }
        } else {
            tx_sample = tx_ok;
            tx_sample_count = 1;
        }

        let prev_rx = last_rx;
        let mut accepted_rx = last_rx;
        if rx_sample_count >= STABLE_SAMPLES && rx_sample != last_rx {
            accepted_rx = rx_sample;
            last_rx = rx_sample;
            rprintln!(
                "[RX] {} -> {} @ {}ms",
                if prev_rx { "OK" } else { "FAIL" },
                if rx_sample { "OK" } else { "FAIL" },
                now_ms
            );
        }

        let prev_tx = last_tx;
        let mut accepted_tx = last_tx;
        if tx_sample_count >= STABLE_SAMPLES && tx_sample != last_tx {
            accepted_tx = tx_sample;
            last_tx = tx_sample;
            rprintln!(
                "[TX] {} -> {} @ {}ms",
                if prev_tx { "OK" } else { "FAIL" },
                if tx_sample { "OK" } else { "FAIL" },
                now_ms
            );
        }

        // Recompute error on accepted states.
        let accepted_err = interpret_err(accepted_rx, accepted_tx);

        if accepted_err != last_err {
            let prev_err = last_err;
            rprintln!(
                "[ERR] 0x{:02x} -> 0x{:02x} ({}) @ {}ms",
                prev_err,
                accepted_err,
                err_reason(accepted_err),
                now_ms
            );
            last_err = accepted_err;
            if accepted_err != 0 {
                sticky_err = Some((accepted_err, now_ms));
                rprintln!("[LATCH] ERR=0x{:02x} @ {}ms", accepted_err, now_ms);
            } else {
                rprintln!("[LATCH] cleared (live) @ {}ms", now_ms);
            }
        }

        // Latch clear via D4 (PG7) or button PI11.
        let latch_clear = gpiog.idr.read().idr7().bit_is_set() || gpioi.idr.read().idr11().bit_is_set();

        if latch_clear {
            if let Some((code, ts)) = sticky_err.take() {
                rprintln!("ERR latch cleared by input (code=0x{:02x}, set @ {}ms)", code, ts);
            }
        } else if let Some((code, ts)) = sticky_err {
            // Auto-clear after 5 seconds.
            if now_ms.wrapping_sub(ts) >= LATCH_AUTO_CLEAR_MS {
                sticky_err = None;
                rprintln!(
                    "ERR latch auto-cleared after {}ms (code=0x{:02x})",
                    LATCH_AUTO_CLEAR_MS,
                    code
                );
            }
        }

        let display_err = sticky_err.map(|(code, _)| code).unwrap_or(accepted_err);
        let display_rx = accepted_rx;
        let display_tx = accepted_tx;

        if display_err != shown_err || display_rx != shown_rx || display_tx != shown_tx {
            unsafe {
                display_update(display_rx, display_tx, display_err);
            }
            shown_err = display_err;
            shown_rx = display_rx;
            shown_tx = display_tx;
        }

        // ~5 ms poll interval without timers (200 MHz core -> ~0.5 ns per nop).
        for _ in 0..1_000_000 {
            asm::nop();
        }
    }
}

#[exception]
fn SysTick() {
    unsafe {
        HAL_IncTick();
        HAL_SYSTICK_IRQHandler();
    }
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    // Re-initialize in case we faulted before `main` ran.
    setup_rtt();

    let scb = &*cortex_m::peripheral::SCB::PTR;
    let cfsr = scb.cfsr.read();
    let hfsr = scb.hfsr.read();
    let mmfar = scb.mmfar.read();
    let bfar = scb.bfar.read();

    rprintln!("HardFault");
    rprintln!("  HFSR=0x{:08x} CFSR=0x{:08x}", hfsr, cfsr);
    rprintln!("  MMFAR=0x{:08x} BFAR=0x{:08x}", mmfar, bfar);
    rprintln!("  PC=0x{:08x} LR=0x{:08x}", ef.pc(), ef.lr());

    loop {
        asm::wfi();
    }
}
