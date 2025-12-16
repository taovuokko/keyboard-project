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

type UsbBusType = hal::usbd::Usbd<hal::usbd::UsbPeripheral<'static>>;

#[entry]
fn main() -> ! {
    let board = hal::pac::Peripherals::take().unwrap();
    let port0 = hal::gpio::p0::Parts::new(board.P0);

    // nice!nano user LED on P0.13.
    let mut led = port0
        .p0_13
        .into_push_pull_output(hal::gpio::Level::Low)
        .degrade();

    // Leak clocks so USB can hold a 'static reference.
    let clocks = hal::clocks::Clocks::new(board.CLOCK)
        .enable_ext_hfosc()
        .start_lfclk();
    let clocks: &'static _ = unsafe {
        CLOCKS = Some(clocks);
        CLOCKS.as_ref().unwrap()
    };

    let (_bus, mut usb_dev, mut usb_serial) = usb_init(board.USBD, &clocks);

    let mut timer = hal::Timer::new(board.TIMER0);
    let mut led_ms: u32 = 0;
    let mut hello_ms: u32 = 0;

    loop {
        // Poll USB often enough for enumeration.
        if usb_dev.poll(&mut [&mut usb_serial]) {
            if hello_ms >= 1000 {
                let _ = usb_serial.write(b"hello usb\r\n");
                hello_ms = 0;
            }
        }

        if led_ms >= 500 {
            let _ = led.set_high();
        }
        if led_ms >= 1000 {
            let _ = led.set_low();
            led_ms = 0;
        }

        timer.delay_ms(1u32);
        led_ms = led_ms.saturating_add(1);
        hello_ms = hello_ms.saturating_add(1);
    }
}

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

    cortex_m::interrupt::free(|_cs| unsafe {
        USB_BUS = Some(UsbBusAllocator::new(usbd));
        let bus = USB_BUS.as_ref().unwrap();
        let serial = SerialPort::new(bus);
        let dev = UsbDeviceBuilder::new(bus, UsbVidPid(0x239a, 0x8029))
            .product("feather-fw")
            .manufacturer("keyboard-project")
            .serial_number("0001")
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();
        (bus, dev, serial)
    })
}
