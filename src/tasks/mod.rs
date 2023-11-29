pub mod api;
mod tag;

pub use tag::*;

use embassy_net::{Ipv4Address, Stack};
use embassy_rp::{
    gpio::Output,
    peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

use crate::animal_tag::AnimalTag;

pub const REMOTE_ENDPOINT: Ipv4Address = include!("../../cfg/ip.rs.inc");
static TAG_SIGNAL: Signal<CriticalSectionRawMutex, AnimalTag> = Signal::new();

#[embassy_executor::task]
pub async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
pub async fn wifi_task(
    runner: cyw43::Runner<
        'static,
        Output<'static, PIN_23>,
        cyw43_pio::PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>,
    >,
) -> ! {
    runner.run().await
}
