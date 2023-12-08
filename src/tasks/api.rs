use alloc::format;
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;

use crate::animal_tag::AnimalTag;

#[embassy_executor::task]
pub async fn receive_task(stack: &'static Stack<cyw43::NetDriver<'static>>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut mb_buf = [0; 4096];
    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        if let Err(e) = socket.accept(6668).await {
            log::error!("Could not accept socket connection: {e:?}");
            socket.close();
            continue;
        }

        if let Some(endpoint) = socket.remote_endpoint() {
            log::info!("Accepted connection from {endpoint}");
        } else {
            log::error!("Could not get remote endpoint");
            socket.close();
            continue;
        }

        let n = match socket.read(&mut mb_buf).await {
            Ok(0) => {
                log::info!("read EOF");
                socket.close();
                continue;
            }
            Ok(n) => {
                log::info!("Received {n} bytes");
                let mut req_headers = [httparse::EMPTY_HEADER; 16];
                let mut req = httparse::Request::new(&mut req_headers);
                let Ok(_) = req.parse(&mb_buf[..n]) else {
                    log::error!("Could not parse HTTP request");
                    continue;
                };
                let text = format!("You accessed {:?}!", req.path);
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
                let n = resp.as_bytes().len();
                mb_buf[0..n].copy_from_slice(resp.as_bytes());
                log::info!("Will send {n} bytes: {resp}");
                n
            }
            Err(e) => {
                log::error!("{:?}", e);
                socket.close();
                continue;
            }
        };

        log::info!("Sending");
        if let Err(e) = socket.write_all(&mb_buf[..n]).await {
            log::error!("write error: {:?}", e);
        }
        log::info!("Sent");
        let _ = socket.flush().await;
        socket.close();
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
