#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, async_fn_in_trait)]

mod animal_tag;

use cyw43::NetDriver;
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::tcp::client::TcpClientState;
use embassy_net::{Config as NetConfig, Stack, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0, UART1, USB};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::uart::{
    Async, Config as UartConfig, DataBits, InterruptHandler as UartInterruptHandler, Parity,
    StopBits, UartRx,
};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::{Duration, Timer};
use futures::TryFutureExt;
use rust_mqtt::client::client::MqttClient;
use rust_mqtt::client::client_config::{ClientConfig, MqttVersion};
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
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    let mut config = UartConfig::default();
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

    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    let net_config = NetConfig::dhcpv4(Default::default());

    let stack = &*make_static!(Stack::new(
        net_device,
        net_config,
        make_static!(StackResources::<2>::new()),
        seed
    ));

    unwrap!(spawner.spawn(net_task(stack)));

    // TODO Move this
    loop {
        match control
            .join_wpa2(include_str!("../ssid.txt"), include_str!("../password.txt"))
            .await
        {
            Ok(_) => break,
            Err(err) => {
                log::info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    log::info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after(Duration::from_millis(100)).await;
    }
    log::info!("DHCP is now up!");
    unwrap!(spawner.spawn(mqtt_task(stack)));

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

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

struct SocketWrapper<'a>(TcpSocket<'a>);
#[derive(Debug)]
struct DummyError;

impl embedded_io::Error for DummyError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl<'a> embedded_io::Io for SocketWrapper<'a> {
    type Error = DummyError;
}

impl<'a> embedded_io::asynch::Read for SocketWrapper<'a> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        embedded_io_async::Read::read(&mut self.0, buf).await.map_err(|_| DummyError)
    }
}

impl<'a> embedded_io::asynch::Write for SocketWrapper<'a> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        embedded_io_async::Write::write(&mut self.0, buf).await.map_err(|_| DummyError)
    }
}

#[embassy_executor::task]
async fn mqtt_task(stack: &'static Stack<cyw43::NetDriver<'static>>) {
    let mut tx_buffer = [0u8; 4096];
    let mut rx_buffer = [0u8; 4096];
    let mut buffer = [0u8; 4096];
    let mut recv_buffer = [0u8; 4096];
    let rng = rand::rngs::mock::StepRng::new(0, 1024);
    let mut network_driver = embassy_net::tcp::TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    let ip = embassy_net::IpAddress::v4(192, 168, 1, 199);
    if let Err(_) = network_driver.connect((ip, 1883)).await {
        log::error!("Could not connect to socket");
        return;
    }
    let config: ClientConfig<'_, 10, rand::rngs::mock::StepRng> = ClientConfig::new(MqttVersion::MQTTv5, rng);
    let mut client = MqttClient::new(SocketWrapper(network_driver), &mut buffer, 4096, &mut recv_buffer, 0, config);
    if let Err(_) = client.connect_to_broker().await {
        log::error!("Could not connect to broker");
        return;
    }
    let delay = Duration::from_secs(1);
    loop {
        if let Err(_) = client
            .send_message(
                "helo",
                b"hello",
                rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS0,
                true,
            )
            .await {
                log::error!("Could not send message");
            }
        Timer::after(delay).await;
    }
}
