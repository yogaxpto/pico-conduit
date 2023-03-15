#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;
use rp2040_hal::{clocks::init_clocks_and_plls, gpio::Pin, pac, sio::Sio};

#[entry]
fn main() -> ! {
    // Initialize clocks and PLLs
    let mut pac = pac::Peripherals::take().unwrap();
    let mut sio = Sio::new(pac.SIO);
    let mut clocks = init_clocks_and_plls(
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut sio,
    )
    .unwrap();

    // Initialize GPIO
    let gpio = pac.GPIO.split(&mut pac.RESETS);
    let mut led_pin = Pin::new(gpio.gpio25).into_output();

    loop {
        // Turn the LED on
        led_pin.set_high().unwrap();

        // Wait for a short period
        delay(100_000);

        // Turn the LED off
        led_pin.set_low().unwrap();

        // Wait for a short period
        delay(100_000);
    }
}

// A simple blocking delay function
fn delay(count: u32) {
    for _ in 0..count {
        cortex_m::asm::nop();
    }
}
