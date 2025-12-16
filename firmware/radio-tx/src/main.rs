#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::digital::v2::OutputPin;
use nrf52840_hal as hal;
use panic_halt as _;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{UsbDevice, UsbDeviceBuilder, UsbVidPid};
use usbd_serial::SerialPort;

const CHANNEL: u8 = 7; // 2407 MHz
const ADDRESS_PREFIX: u8 = 0xE7;
const ADDRESS_BASE: u32 = 0xE7E7E7E7;
const PACKET_LEN: usize = 8;

type UsbBusType = hal::usbd::Usbd<hal::usbd::UsbPeripheral<'static>>;

#[entry]
fn main() -> ! {
    let p = hal::pac::Peripherals::take().unwrap();
    let port0 = hal::gpio::p0::Parts::new(p.P0);

    // LED on nice!nano P0.13
    let mut led = port0
        .p0_13
        .into_push_pull_output(hal::gpio::Level::Low)
        .degrade();

    let clocks = hal::clocks::Clocks::new(p.CLOCK)
        .enable_ext_hfosc()
        .start_lfclk();
    // SAFETY: This is the only write to CLOCKS, happens before any interrupts are enabled,
    // and the reference is valid for 'static as it references the hardware CLOCK peripheral.
    let clocks: &'static _ = unsafe {
        CLOCKS = Some(clocks);
        CLOCKS.as_ref().unwrap()
    };
    let mut timer = hal::Timer::new(p.TIMER0);

    let mut radio = p.RADIO;
    setup_radio(&mut radio);

    let (_bus, mut usb_dev, mut usb_serial) = usb_init(p.USBD, clocks);

    let mut seq: u8 = 0;
    let mut buf = [0u8; PACKET_LEN];
    let mut usb_ms: u32 = 0;

    loop {
        buf = *b"PINGPONG";
        buf[7] = seq;
        seq = seq.wrapping_add(1);

        tx_packet(&mut radio, &buf);

        if usb_dev.poll(&mut [&mut usb_serial]) {
            if usb_ms >= 500 {
                let _ = usb_serial.write(b"tx seq ");
                let _ = usb_serial.write(&[buf[7]]);
                let _ = usb_serial.write(b"\r\n");
                usb_ms = 0;
            }
        }

        // blink fast to show tx loop alive
        let _ = led.set_high();
        timer.delay_ms(50u32);
        let _ = led.set_low();
        timer.delay_ms(200u32);
        usb_ms = usb_ms.saturating_add(250);
    }
}

// SAFETY: These statics are only written to during initialization (before interrupts are enabled)
// and then only read. The 'static references are valid for the program's lifetime since they
// reference hardware peripherals.
static mut USB_BUS: Option<UsbBusAllocator<UsbBusType>> = None;
static mut CLOCKS: Option<
    hal::clocks::Clocks<
        hal::clocks::ExternalOscillator,
        hal::clocks::Internal,
        hal::clocks::LfOscStarted,
    >,
> = None;

fn usb_init(
    usbd: hal::pac::USBD,
    clocks: &'static hal::clocks::Clocks<
        hal::clocks::ExternalOscillator,
        hal::clocks::Internal,
        hal::clocks::LfOscStarted,
    >,
) -> (
    &'static UsbBusAllocator<UsbBusType>,
    UsbDevice<'static, UsbBusType>,
    SerialPort<'static, UsbBusType>,
) {
    let periph = hal::usbd::UsbPeripheral::new(usbd, clocks);
    let usbd = hal::usbd::Usbd::new(periph);

    // SAFETY: This is the only write to USB_BUS, happens within an interrupt-free critical section,
    // and is only called once during initialization. The references are valid for 'static as they
    // reference the USB peripheral which exists for the program's lifetime.
    cortex_m::interrupt::free(|_cs| unsafe {
        USB_BUS = Some(UsbBusAllocator::new(usbd));
        let bus = USB_BUS.as_ref().unwrap();
        let serial = SerialPort::new(bus);
        let dev = UsbDeviceBuilder::new(bus, UsbVidPid(0x239a, 0x8029))
            .product("radio-tx")
            .manufacturer("keyboard-project")
            .serial_number("0001")
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();
        (bus, dev, serial)
    })
}

fn setup_radio(radio: &mut hal::pac::RADIO) {
    radio.power.write(|w| w.power().enabled());

    // 2 Mbps mode, whitening on.
    radio.mode.write(|w| w.mode().nrf_2mbit());
    radio.txpower.write(|w| {
        w.txpower()
            .variant(hal::pac::radio::txpower::TXPOWER_A::_0D_BM)
    });
    radio
        .frequency
        .write(|w| unsafe { w.frequency().bits(CHANNEL) });

    // Address config
    radio.base0.write(|w| unsafe { w.bits(ADDRESS_BASE) });
    radio
        .prefix0
        .write(|w| unsafe { w.ap0().bits(ADDRESS_PREFIX) });
    radio.txaddress.write(|w| unsafe { w.txaddress().bits(0) });
    radio.rxaddresses.write(|w| w.addr0().enabled());

    // Packet configuration: fixed length = PACKET_LEN, no S0/S1, little endian, whitening.
    radio.pcnf0.write(|w| unsafe {
        w.lflen().bits(0); // no length field in buffer
        w.s0len().bit(false);
        w.s1len().bits(0);
        w.s1incl().clear_bit();
        w.plen()._8bit();
        w.crcinc().clear_bit()
    });
    radio.pcnf1.write(|w| unsafe {
        w.maxlen().bits(PACKET_LEN as u8);
        w.statlen().bits(PACKET_LEN as u8); // fixed-length packets
        w.balen().bits(4); // base address length
        w.endian().little();
        w.whiteen().enabled()
    });

    // CRC 2 bytes, init 0xFFFF
    radio.crccnf.write(|w| w.len().two());
    radio.crcinit.write(|w| unsafe { w.bits(0xFFFF) });
    radio.crcpoly.write(|w| unsafe { w.bits(0x11021) });
}

fn tx_packet(radio: &mut hal::pac::RADIO, buf: &[u8; PACKET_LEN]) {
    // Ensure DISABLED state
    radio.events_disabled.reset();
    radio.tasks_disable.write(|w| unsafe { w.bits(1) });
    while radio.events_disabled.read().bits() == 0 {}
    radio.events_disabled.reset();

    radio
        .packetptr
        .write(|w| unsafe { w.packetptr().bits(buf.as_ptr() as u32) });
    radio.events_ready.reset();
    radio.events_end.reset();

    radio.tasks_txen.write(|w| unsafe { w.bits(1) });
    while radio.events_ready.read().bits() == 0 {}
    radio.events_ready.reset();

    radio.tasks_start.write(|w| unsafe { w.bits(1) });
    while radio.events_end.read().bits() == 0 {}
    radio.events_end.reset();

    radio.tasks_disable.write(|w| unsafe { w.bits(1) });
    while radio.events_disabled.read().bits() == 0 {}
    radio.events_disabled.reset();
}
