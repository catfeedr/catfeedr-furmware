#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod allocator;
mod animal_tag;
mod net_logger;
mod tasks;

extern crate alloc;
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{Config as NetConfig, Stack, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{PIO0, UART1};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::uart::{
    Config as UartConfig, DataBits, InterruptHandler as UartInterruptHandler, Parity, StopBits,
    UartRx,
};

use embassy_time::{Duration, Timer};
use static_cell::make_static;

use crate::net_logger::net_logger_task;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

bind_interrupts!(struct UartIrqs {
    UART1_IRQ => UartInterruptHandler<UART1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let fw = include_bytes!("../embassy/cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../embassy/cyw43-firmware/43439A0_clm.bin");

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
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(tasks::wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    let net_config = NetConfig::dhcpv4(Default::default());

    let stack = &*make_static!(Stack::new(
        net_device,
        net_config,
        make_static!(StackResources::<4>::new()),
        seed
    ));

    unwrap!(spawner.spawn(tasks::net_task(stack)));

    // TODO Move this
    loop {
        match control
            .join_wpa2(
                include_str!("../cfg/ssid.txt"),
                include_str!("../cfg/password.txt"),
            )
            .await
        {
            Ok(_) => break,
            Err(err) => {
                log::info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    while !stack.is_config_up() {
        Timer::after(Duration::from_millis(100)).await;
    }

    spawner.spawn(net_logger_task(stack)).unwrap();

    let mut config = UartConfig::default();
    config.baudrate = 9600;
    config.data_bits = DataBits::DataBits8;
    config.parity = Parity::ParityNone;
    config.stop_bits = StopBits::STOP2;
    let uart_rx = UartRx::new(p.UART1, p.PIN_5, UartIrqs, p.DMA_CH1, config);
    unwrap!(spawner.spawn(tasks::tag_reader_task(uart_rx)));

    unwrap!(spawner.spawn(tasks::api::response_task(stack)));
    unwrap!(spawner.spawn(tasks::api::receive_task(stack)));

    let delay = Duration::from_secs(1);

    loop {
        control.gpio_set(0, true).await;
        Timer::after(delay).await;
        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}
