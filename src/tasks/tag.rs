use embassy_rp::{
    peripherals::UART1,
    uart::{Async, UartRx},
};
use embassy_time::{Duration, Timer};

use crate::animal_tag::AnimalTag;

#[embassy_executor::task]
pub async fn tag_reader_task(mut rx: UartRx<'static, UART1, Async>) {
    log::info!("reading");
    let delay = Duration::from_secs(1);
    loop {
        let mut buf = [0u8; 30];
        rx.read(&mut buf).await.unwrap();

        let tag: AnimalTag = buf.into();
        log::info!("Got card ID: {}", tag.id());
        super::TAG_SIGNAL.signal(tag);
        Timer::after(delay).await;
    }
}
