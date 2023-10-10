//! This example test the RP Pico W on board LED.
//!
//! It does not work with the RP Pico board. See blinky.rs.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod animal_tag;

use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0, UART1, USB};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::uart::{
    Async, Config, DataBits, InterruptHandler as UartInterruptHandler, Parity, StopBits, UartRx,
};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::{Duration, Timer};
use embedded_io_async::Read;
use static_cell::make_static;

use crate::animal_tag::AnimalTag;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

bind_interrupts!(struct UartIrqs {
    UART1_IRQ => UartInterruptHandler<UART1>;
});

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
});

#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<
        'static,
        Output<'static, PIN_23>,
        PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>,
    >,
) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let fw = include_bytes!("../embassy/cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../embassy/cyw43-firmware/43439A0_clm.bin");

    let driver = Driver::new(p.USB, UsbIrqs);
    spawner.spawn(logger_task(driver)).unwrap();

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    let state = make_static!(cyw43::State::new());
    let (_net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    let mut config = Config::default();
    config.baudrate = 9600;
    config.data_bits = DataBits::DataBits8;
    config.parity = Parity::ParityNone;
    config.stop_bits = StopBits::STOP2;
    let uart_rx = UartRx::new(p.UART1, p.PIN_5, UartIrqs, p.DMA_CH1, config);
    unwrap!(spawner.spawn(reader(uart_rx)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let delay = Duration::from_secs(1);
    loop {
        control.gpio_set(0, true).await;
        Timer::after(delay).await;

        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}

#[embassy_executor::task]
async fn reader(mut rx: UartRx<'static, UART1, Async>) {
    info!("reading");
    let delay = Duration::from_secs(1);
    loop {
        let mut buf = [0u8; 30];
        rx.read(&mut buf).await.unwrap();
        log::info!("RX: {:?}", buf);

        let tag: AnimalTag = unsafe { core::mem::transmute_copy(&buf) };
        log::info!("Card number: {}", tag.card_number().as_str());
        Timer::after(delay).await;
    }
}

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}
