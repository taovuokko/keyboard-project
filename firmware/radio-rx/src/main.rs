#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::digital::v2::OutputPin;
use nrf52840_hal as hal;
use panic_halt as _;
use usbd_serial::SerialPort;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{UsbDevice, UsbDeviceBuilder, UsbVidPid};

const CHANNEL: u8 = 7; // 2407 MHz
const ADDRESS_PREFIX: u8 = 0xE7;
const ADDRESS_BASE: u32 = 0xE7E7E7E7;
const PACKET_LEN: usize = 8;

type UsbBusType = hal::usbd::Usbd<hal::usbd::UsbPeripheral<'static>>;
static mut CLOCKS: Option<
    hal::clocks::Clocks<
        hal::clocks::ExternalOscillator,
        hal::clocks::Internal,
        hal::clocks::LfOscStarted,
    >,
> = None;
static mut USB_BUS: Option<UsbBusAllocator<UsbBusType>> = None;

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
    let clocks: &'static _ = unsafe {
        CLOCKS = Some(clocks);
        CLOCKS.as_ref().unwrap()
    };
    let mut timer = hal::Timer::new(p.TIMER0);

    let mut radio = p.RADIO;
    setup_radio(&mut radio);

    let mut buf = [0u8; PACKET_LEN];
    let mut blink_ms: u32 = 0;
    let mut packets: u32 = 0;

    // Lazy USB init after some delay to avoid early crashes.
    let mut usb_ready = false;
    let mut usb_dev: Option<UsbDevice<'static, UsbBusType>> = None;
    let mut usb_serial: Option<SerialPort<'static, UsbBusType>> = None;
    let mut usbd_periph = Some(p.USBD);
    static mut USB_BUS: Option<UsbBusAllocator<UsbBusType>> = None;

    loop {
        if let Some(rx_ok) = rx_packet(&mut radio, &mut buf) {
            if rx_ok {
                let _ = led.set_high();
                timer.delay_ms(50u32);
                let _ = led.set_low();
                packets = packets.wrapping_add(1);

                if usb_ready {
                    if let (Some(dev), Some(serial)) = (usb_dev.as_mut(), usb_serial.as_mut()) {
                        if dev.poll(&mut [serial]) {
                            log_packet(serial, buf[7], packets);
                        }
                    }
                }
            }
        }

        // idle blink each second so we know we are alive even without packets
        if blink_ms >= 1000 {
            let _ = led.set_high();
            timer.delay_ms(50u32);
            let _ = led.set_low();
            blink_ms = 0;
        }

        timer.delay_ms(1u32);
        blink_ms = blink_ms.saturating_add(1);

        if !usb_ready && blink_ms > 300 {
            if let Some(usbd) = usbd_periph.take() {
                if let Some((dev, serial)) = usb_init(usbd, clocks) {
                    usb_dev = Some(dev);
                    usb_serial = Some(serial);
                    usb_ready = true;
                }
            }
        }
    }
}

fn usb_init(
    usbd: hal::pac::USBD,
    clocks: &'static hal::clocks::Clocks<
        hal::clocks::ExternalOscillator,
        hal::clocks::Internal,
        hal::clocks::LfOscStarted,
    >,
) -> Option<(UsbDevice<'static, UsbBusType>, SerialPort<'static, UsbBusType>)> {
    let periph = hal::usbd::UsbPeripheral::new(usbd, clocks);
    let usbd = hal::usbd::Usbd::new(periph);

    let (dev, serial) = cortex_m::interrupt::free(|_| unsafe {
        USB_BUS = Some(UsbBusAllocator::new(usbd));
        let bus = USB_BUS.as_ref().unwrap();
        let serial = SerialPort::new(bus);
        let dev = UsbDeviceBuilder::new(bus, UsbVidPid(0x239a, 0x8029))
            .product("radio-rx")
            .manufacturer("keyboard-project")
            .serial_number("0002")
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();
        (dev, serial)
    });
    Some((dev, serial))
}

fn log_packet(serial: &mut SerialPort<'static, UsbBusType>, seq: u8, total: u32) {
    let _ = serial.write(b"rx pkt ");
    let _ = serial.write(&[seq]);
    let _ = serial.write(b" total=");
    let mut n = total;
    let mut digits = [0u8; 10];
    let mut i = 10;
    if n == 0 {
        i -= 1;
        digits[i] = b'0';
    } else {
        while n > 0 && i > 0 {
            i -= 1;
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
    }
    let _ = serial.write(&digits[i..]);
    let _ = serial.write(b"\r\n");
}

fn setup_radio(radio: &mut hal::pac::RADIO) {
    radio.power.write(|w| w.power().enabled());

    // 2 Mbps mode, whitening on.
    radio.mode.write(|w| w.mode().nrf_2mbit());
    radio
        .txpower
        .write(|w| w.txpower().variant(hal::pac::radio::txpower::TXPOWER_A::_0D_BM));
    radio.frequency.write(|w| unsafe { w.frequency().bits(CHANNEL) });

    // Address config
    radio.base0.write(|w| unsafe { w.bits(ADDRESS_BASE) });
    radio.prefix0.write(|w| unsafe { w.ap0().bits(ADDRESS_PREFIX) });
    radio.txaddress.write(|w| unsafe { w.txaddress().bits(0) });
    radio.rxaddresses.write(|w| w.addr0().enabled());

    // Packet configuration: no S0/S1, 8-bit length, little endian, whitening, max PACKET_LEN.
    radio.pcnf0.write(|w| unsafe {
        w.lflen().bits(8);
        w.s0len().bit(false);
        w.s1len().bits(0);
        w.s1incl().clear_bit();
        w.plen()._8bit();
        w.crcinc().clear_bit()
    });
    radio.pcnf1.write(|w| unsafe {
        w.maxlen().bits(PACKET_LEN as u8);
        w.statlen().bits(0);
        w.balen().bits(4);
        w.endian().little();
        w.whiteen().enabled()
    });

    // CRC 2 bytes, init 0xFFFF
    radio.crccnf.write(|w| w.len().two());
    radio.crcinit.write(|w| unsafe { w.bits(0xFFFF) });
    radio.crcpoly.write(|w| unsafe { w.bits(0x11021) });
}

fn rx_packet(radio: &mut hal::pac::RADIO, buf: &mut [u8; PACKET_LEN]) -> Option<bool> {
    radio.events_disabled.reset();
    radio.events_end.reset();
    radio.events_ready.reset();
    radio.events_crcok.reset();

    radio.packetptr.write(|w| unsafe { w.packetptr().bits(buf.as_mut_ptr() as u32) });

    radio.tasks_rxen.write(|w| unsafe { w.bits(1) });
    while radio.events_ready.read().bits() == 0 {}
    radio.events_ready.reset();

    radio.tasks_start.write(|w| unsafe { w.bits(1) });
    while radio.events_end.read().bits() == 0 {}
    radio.events_end.reset();

    let crc_ok = radio.events_crcok.read().bits() != 0;
    radio.events_crcok.reset();

    radio.tasks_disable.write(|w| unsafe { w.bits(1) });
    while radio.events_disabled.read().bits() == 0 {}
    radio.events_disabled.reset();

    Some(crc_ok)
}
