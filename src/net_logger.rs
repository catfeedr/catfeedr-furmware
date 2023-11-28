use core::fmt::Write as _;

use embassy_net::{tcp::TcpSocket, Ipv4Address};
use embassy_sync::pipe::Pipe;
use log::{Metadata, Record};

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
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));
        let remote_endpoint = (address, port);
        if socket.connect(remote_endpoint).await.is_err() {
            socket.close();
        }

        let mut rx: [u8; MAX_PACKET_SIZE as usize] = [0; MAX_PACKET_SIZE as usize];
        loop {
            let len = self.buffer.read(&mut rx[..]).await;
            let _ = socket.write(&rx[..len]).await;
        }
    }
}

impl<const N: usize> log::Log for NetLogger<N> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = write!(Writer(&self.buffer), "{}\r\n", record.args());
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
