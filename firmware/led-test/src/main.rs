#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::digital::v2::OutputPin;
use nrf52840_hal as hal;
use panic_halt as _;

#[entry]
fn main() -> ! {
    let p = hal::pac::Peripherals::take().unwrap();
    let port0 = hal::gpio::p0::Parts::new(p.P0);

    // nice!nano user LED on P0.13
    let mut led = port0
        .p0_13
        .into_push_pull_output(hal::gpio::Level::Low)
        .degrade();

    let _clocks = hal::clocks::Clocks::new(p.CLOCK)
        .enable_ext_hfosc()
        .start_lfclk();

    let mut timer = hal::Timer::new(p.TIMER0);

    loop {
        let _ = led.set_high();
        timer.delay_ms(300u32);
        let _ = led.set_low();
        timer.delay_ms(300u32);
    }
}
