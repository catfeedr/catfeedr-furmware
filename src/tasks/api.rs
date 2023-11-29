use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;

use crate::animal_tag::AnimalTag;

#[embassy_executor::task]
pub async fn receive_task(stack: &'static Stack<cyw43::NetDriver<'static>>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut mb_buf = [0; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    if let Err(e) = socket.accept(6668).await {
        log::error!("Could not accept socket connection: {e:?}");
    }

    if let Some(endpoint) = socket.remote_endpoint() {
        log::info!("Accepted connection from {endpoint}");
    }

    loop {
        let n = match socket.read(&mut mb_buf).await {
            Ok(0) => {
                log::info!("read EOF");
                break;
            }
            Ok(n) => n,
            Err(e) => {
                log::error!("{:?}", e);
                break;
            }
        };

        if let Err(e) = socket.write_all(&mb_buf[..n]).await {
            log::error!("write error: {:?}", e);
            break;
        }
    }
}

#[embassy_executor::task]
pub async fn response_task(stack: &'static Stack<cyw43::NetDriver<'static>>) {
    let mut tag: Option<AnimalTag> = None;
    'reconnect: loop {
        let mut rx_buffer = [0; 4096];
        let mut tx_buffer = [0; 4096];
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        let remote_endpoint = (super::REMOTE_ENDPOINT, 6666);
        log::info!(
            "connecting to {}:{}...",
            remote_endpoint.0,
            remote_endpoint.1
        );
        if socket.connect(remote_endpoint).await.is_err() {
            log::error!("failed to connect");
            socket.close();
            Timer::after(Duration::from_secs(1)).await;
            continue 'reconnect;
        }
        log::info!("connected!");

        loop {
            tag = tag.or(Some(super::TAG_SIGNAL.wait().await));
            if socket
                .write_all(tag.unwrap().id().as_bytes())
                .await
                .is_err()
            {
                log::error!("Could not write tag. Reconnecting.");
                socket.close();
                continue 'reconnect;
            }
            tag = None;
        }
    }
}
