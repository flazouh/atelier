use std::{
    io::{Read, Write},
    net::{TcpStream},
    time::{Duration},
};

use rustls::{ClientConnection, StreamOwned};

pub(super) trait Wire: Read + Write + Send {
    fn wake_after(&self, timeout: Duration);
    fn close(&self);
}

impl Wire for TcpStream {
    fn wake_after(&self, timeout: Duration) {
        let _ = self.set_read_timeout(Some(timeout));
    }

    fn close(&self) {
        let _ = self.shutdown(std::net::Shutdown::Both);
    }
}

impl Wire for StreamOwned<ClientConnection, TcpStream> {
    fn wake_after(&self, timeout: Duration) {
        let _ = self.sock.set_read_timeout(Some(timeout));
    }

    fn close(&self) {
        let _ = self.sock.shutdown(std::net::Shutdown::Both);
    }
}
