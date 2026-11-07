//! Owns bounded blocking exchanges and reconnects with the caller's unchanged request id.

use crate::{Command, Response, read_response, write_request};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub struct Client {
    address: SocketAddr,
    timeout: Duration,
    attempts: u32,
}

impl Client {
    /// Timeout applies separately to connect, whole-frame write and whole-frame read.
    /// An IO error after all attempts means the result is unknown, not aborted.
    pub fn new(address: SocketAddr, timeout: Duration, attempts: u32) -> io::Result<Self> {
        if !address.ip().is_loopback() || timeout.is_zero() || attempts == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "need loopback, nonzero timeout and attempts",
            ));
        }
        Ok(Self {
            address,
            timeout,
            attempts,
        })
    }

    pub fn call(&self, command: &Command) -> io::Result<Response> {
        for attempt in 0..self.attempts {
            let result = self.once(command);
            match result {
                Err(e)
                    if attempt + 1 < self.attempts
                        && matches!(
                            e.kind(),
                            io::ErrorKind::ConnectionRefused
                                | io::ErrorKind::ConnectionReset
                                | io::ErrorKind::ConnectionAborted
                                | io::ErrorKind::BrokenPipe
                                | io::ErrorKind::TimedOut
                                | io::ErrorKind::WouldBlock
                                | io::ErrorKind::UnexpectedEof
                        ) => {}
                result => return result,
            }
        }
        Err(io::Error::other("no exchange attempted"))
    }

    fn once(&self, command: &Command) -> io::Result<Response> {
        let mut stream = TcpStream::connect_timeout(&self.address, self.timeout)?;
        stream.set_nodelay(true)?;
        write_request(&mut stream, command, self.timeout)?;
        read_response(&mut stream, self.timeout)
    }
}
