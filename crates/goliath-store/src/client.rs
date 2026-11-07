//! Owns bounded blocking exchanges and reconnects with the caller's unchanged request id.

use crate::{Command, Response, read_response, write_request};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Successful attempt phases. Retry time is included only in the caller's clock.
pub struct CallTiming {
    pub connect: Duration,
    pub write: Duration,
    pub read: Duration,
}

/// Process-unique request identity. Retain the complete Command for uncertain retries.
pub fn request_id() -> crate::RequestId {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    static START: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let start = *START.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    });
    crate::RequestId(
        start
            .wrapping_add(u128::from(std::process::id()) << 64)
            .wrapping_add(u128::from(NEXT.fetch_add(1, Ordering::Relaxed))),
    )
}

pub struct Client {
    address: SocketAddr,
    timeout: Duration,
    attempts: u32,
    stream: Mutex<Option<TcpStream>>,
    reuse: bool,
}

impl Client {
    /// Uses a fresh request identity and anchors the keeper before any network IO.
    pub fn claim_lease(
        &self,
        tile: crate::TileId,
        lease_ms: u64,
        margin: Duration,
    ) -> io::Result<crate::LeaseKeeper> {
        let mut keeper = crate::LeaseKeeper::new(tile, lease_ms, margin).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid lease or margin")
        })?;
        let request = keeper.request(Instant::now()).expect("fresh lease keeper");
        let response = self.execute(crate::Operation::ClaimLease { tile, lease_ms })?;
        if let Response::Leased(grant) = response
            && keeper.observe(request, grant, Instant::now())
        {
            return Ok(keeper);
        }
        Err(io::Error::other(format!(
            "lease claim refused or late: {response:?}"
        )))
    }

    /// IO uncertainty leaves the old deadline intact. Never act while waiting
    /// unless a separate side-effect path still checks that deadline itself.
    pub fn renew_lease(&self, keeper: &mut crate::LeaseKeeper) -> io::Result<bool> {
        let Some(fence) = keeper.fence() else {
            return Ok(false);
        };
        let Some(request) = keeper.request(Instant::now()) else {
            return Ok(false);
        };
        let response = self.execute(crate::Operation::RenewLease {
            fence,
            counter: request.counter(),
        })?;
        if let Response::Leased(grant) = response {
            return Ok(keeper.observe(request, grant, Instant::now()));
        }
        keeper.stop();
        Ok(false)
    }

    pub fn execute(&self, operation: crate::Operation) -> io::Result<Response> {
        self.call(&Command {
            id: request_id(),
            operation,
        })
    }
    /// Timeout applies separately to connect, whole-frame write and whole-frame read.
    /// An IO error after all attempts means the result is unknown, not aborted.
    /// Each handle reuses one connection and serializes its own calls.
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
            stream: Mutex::new(None),
            reuse: true,
        })
    }

    /// Original fresh-connection transport, retained for compatibility checks.
    pub fn fresh(address: SocketAddr, timeout: Duration, attempts: u32) -> io::Result<Self> {
        let mut client = Self::new(address, timeout, attempts)?;
        client.reuse = false;
        Ok(client)
    }

    pub fn call(&self, command: &Command) -> io::Result<Response> {
        self.call_with_timing(command).map(|(response, _)| response)
    }

    pub fn call_with_timing(&self, command: &Command) -> io::Result<(Response, CallTiming)> {
        if !self.reuse {
            return self.attempt(command, &mut None);
        }
        let mut stream = self
            .stream
            .lock()
            .map_err(|_| io::Error::other("client connection lock poisoned"))?;
        self.attempt(command, &mut stream)
    }

    fn attempt(
        &self,
        command: &Command,
        stream: &mut Option<TcpStream>,
    ) -> io::Result<(Response, CallTiming)> {
        for attempt in 0..self.attempts {
            let result = self.once(command, stream);
            if result.is_err() {
                // Never reuse a partially written request or partially read reply.
                // An uncertain mutation is retried with exactly the same command.
                *stream = None;
            }
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

    fn once(
        &self,
        command: &Command,
        cached: &mut Option<TcpStream>,
    ) -> io::Result<(Response, CallTiming)> {
        let mut connect = Duration::ZERO;
        if cached.is_none() {
            let begin = Instant::now();
            let stream = TcpStream::connect_timeout(&self.address, self.timeout)?;
            stream.set_nodelay(true)?;
            connect = begin.elapsed();
            *cached = Some(stream);
        }
        let stream = cached.as_mut().expect("connected stream");
        let begin = Instant::now();
        write_request(stream, command, self.timeout)?;
        let write = begin.elapsed();
        let begin = Instant::now();
        let response = read_response(stream, self.timeout)?;
        Ok((
            response,
            CallTiming {
                connect,
                write,
                read: begin.elapsed(),
            },
        ))
    }
}
