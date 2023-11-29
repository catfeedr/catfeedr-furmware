use core::fmt::Write as _;

use embassy_net::{tcp::TcpSocket, Ipv4Address, Stack};
use embassy_sync::pipe::Pipe;
use embassy_time::{Duration, Timer};
use log::{Metadata, Record};

use crate::tasks;

type CS = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

/// The logger handle, which contains a pipe with configurable size for buffering log messages.
pub struct NetLogger<const N: usize> {
    buffer: Pipe<CS, N>,
}

impl<const N: usize> NetLogger<N> {
    pub const fn new() -> Self {
        Self {
            buffer: Pipe::new(),
        }
    }

    pub async fn run<'d>(&'d self, mut socket: TcpSocket<'_>, address: Ipv4Address, port: u16) -> !
    where
        Self: 'd,
    {
        const MAX_PACKET_SIZE: u16 = 1024;
        let remote_endpoint = (address, port);
        'reconnect: loop {
            if socket.connect(remote_endpoint).await.is_err() {
                socket.close();
                Timer::after(Duration::from_secs(1)).await;
                continue 'reconnect;
            }

            log::info!("Logger is up: {}", socket.local_endpoint().unwrap().addr);

            let mut rx: [u8; MAX_PACKET_SIZE as usize] = [0; MAX_PACKET_SIZE as usize];
            loop {
                let len = self.buffer.read(&mut rx[..]).await;
                if socket.write(&rx[..len]).await.is_err() {
                    Timer::after(Duration::from_secs(1)).await;
                    continue 'reconnect;
                }
            }
        }
    }
}

impl<const N: usize> log::Log for NetLogger<N> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = write!(
                Writer(&self.buffer),
                "[{}] ({}:{}): {}\r\n",
                record.level(),
                record.file().unwrap_or("<unknown file>"),
                record.line().unwrap_or_default(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

struct Writer<'d, const N: usize>(&'d Pipe<CS, N>);

impl<'d, const N: usize> core::fmt::Write for Writer<'d, N> {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        let _ = self.0.try_write(s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! run {
    ( $x:expr, $s:expr, $l:expr, $a:expr, $p:expr) => {
        static LOGGER: $crate::net_logger::NetLogger<$x> = $crate::net_logger::NetLogger::new();
        unsafe {
            let _ = ::log::set_logger_racy(&LOGGER).map(|()| log::set_max_level_racy($l));
        }
        let _ = LOGGER.run($s, $a, $p).await;
    };
}

#[embassy_executor::task]
pub async fn net_logger_task(stack: &'static Stack<cyw43::NetDriver<'static>>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    run!(
        1024,
        socket,
        log::LevelFilter::Info,
        tasks::REMOTE_ENDPOINT,
        6667
    );
}
