//! Owns bounded blocking exchanges and reconnects with the caller's unchanged request id.

use crate::{Command, Response};
use std::fs::File;
use std::io::{self, Write};
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
    trace: Option<Mutex<File>>,
}

#[derive(Clone, Copy)]
struct RetryBudget {
    deadline: Instant,
    phase_timeout: Duration,
}

impl RetryBudget {
    fn limit(mut self, deadline: Instant) -> Self {
        self.deadline = self.deadline.min(deadline);
        self
    }
    fn new(start: Instant, phase_timeout: Duration, attempts: u32) -> io::Result<Self> {
        let span = phase_timeout
            .checked_mul(3)
            .and_then(|span| span.checked_mul(attempts))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "client retry budget overflows")
            })?;
        let deadline = start.checked_add(span).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "client retry deadline overflows",
            )
        })?;
        Ok(Self {
            deadline,
            phase_timeout,
        })
    }

    fn remaining(self, now: Instant) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(now)
            .filter(|left| !left.is_zero())
            .ok_or_else(result_unknown)
    }

    fn phase_deadline(self, now: Instant) -> io::Result<Instant> {
        let left = self.remaining(now)?;
        Ok(now + self.phase_timeout.min(left))
    }
}

fn result_unknown() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "result unknown: overall client deadline elapsed",
    )
}

fn exchanges<T>(
    attempts: u32,
    budget: RetryBudget,
    mut now: impl FnMut() -> Instant,
    mut exchange: impl FnMut(u32) -> io::Result<T>,
) -> io::Result<T> {
    for attempt in 0..attempts {
        budget.remaining(now())?;
        let result = exchange(attempt);
        budget.remaining(now())?;
        match result {
            Err(error)
                if attempt + 1 < attempts
                    && matches!(
                        error.kind(),
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
    Err(io::Error::other("result unknown: no exchange attempted"))
}

impl Client {
    /// Diagnostic-only call/lock trace for one handle. The caller owns its file.
    /// No payload bytes are recorded and no retry or authorization rule changes.
    pub fn trace_calls_to(&mut self, mut file: File) -> io::Result<()> {
        writeln!(file, "wall_ns,event,request_id,operation,attempt,instant")?;
        self.trace = Some(Mutex::new(file));
        Ok(())
    }

    fn trace(&self, event: &str, command: &Command, attempt: u32) {
        let Some(trace) = &self.trace else { return };
        let operation = match command.operation {
            crate::Operation::ClaimLease { .. } => "claim-lease",
            crate::Operation::RenewLease { .. } => "renew-lease",
            crate::Operation::Directory => "directory",
            crate::Operation::CheckLease(_) => "check-lease",
            crate::Operation::BeginTakeover(_) => "begin-takeover",
            _ => "other",
        };
        let now = Instant::now();
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        if let Ok(mut trace) = trace.lock()
            && let Err(error) = writeln!(
                trace,
                "{wall},{event},{},{operation},{attempt},\"{now:?}\"",
                command.id.0
            )
        {
            eprintln!("client diagnostic trace failed: {error}");
        }
    }

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
    /// Each phase is capped by timeout. The whole call also has an overall
    /// attempts * 3 * timeout deadline, including mutex waiting and reconnects.
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
            trace: None,
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

    /// Cap the entire call, including handle waiting, reconnects and retries,
    /// by the caller's absolute deadline. A timeout means result unknown.
    /// A process pause cannot be preempted; expiry is rechecked on resumption.
    pub fn call_before(&self, command: &Command, deadline: Instant) -> io::Result<Response> {
        let budget = RetryBudget::new(Instant::now(), self.timeout, self.attempts)?.limit(deadline);
        self.call_with_budget(command, budget)
            .map(|(response, _)| response)
    }

    pub fn call_with_timing(&self, command: &Command) -> io::Result<(Response, CallTiming)> {
        let budget = RetryBudget::new(Instant::now(), self.timeout, self.attempts)?;
        self.call_with_budget(command, budget)
    }

    fn call_with_budget(
        &self,
        command: &Command,
        budget: RetryBudget,
    ) -> io::Result<(Response, CallTiming)> {
        self.trace("call-waiting", command, 0);
        if !self.reuse {
            return self.attempt(command, &mut None, budget);
        }
        let mut stream = loop {
            budget.remaining(Instant::now())?;
            match self.stream.try_lock() {
                Ok(stream) => break stream,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(io::Error::other("client connection lock poisoned"));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(
                        budget
                            .remaining(Instant::now())?
                            .min(Duration::from_millis(1)),
                    );
                }
            }
        };
        self.trace("lock-acquired", command, 0);
        let result = self.attempt(command, &mut stream, budget);
        self.trace("lock-releasing", command, 0);
        result
    }

    fn attempt(
        &self,
        command: &Command,
        stream: &mut Option<TcpStream>,
        budget: RetryBudget,
    ) -> io::Result<(Response, CallTiming)> {
        exchanges(self.attempts, budget, Instant::now, |attempt| {
            self.trace("attempt-start", command, attempt);
            let result = self.once(command, stream, budget);
            if result.is_err() {
                self.trace("attempt-error", command, attempt);
                // Never reuse a partially written request or partially read reply.
                // An uncertain mutation is retried with exactly the same command.
                *stream = None;
            }
            result
        })
    }

    fn once(
        &self,
        command: &Command,
        cached: &mut Option<TcpStream>,
        budget: RetryBudget,
    ) -> io::Result<(Response, CallTiming)> {
        let mut connect = Duration::ZERO;
        if cached.is_none() {
            let begin = Instant::now();
            let stream = TcpStream::connect_timeout(
                &self.address,
                self.timeout.min(budget.remaining(Instant::now())?),
            )?;
            stream.set_nodelay(true)?;
            connect = begin.elapsed();
            *cached = Some(stream);
        }
        let stream = cached.as_mut().expect("connected stream");
        let begin = Instant::now();
        crate::codec::write_request_before(
            stream,
            command,
            budget.phase_deadline(Instant::now())?,
        )?;
        self.trace("request-written", command, 0);
        let write = begin.elapsed();
        let begin = Instant::now();
        let response =
            crate::codec::read_response_before(stream, budget.phase_deadline(Instant::now())?)?;
        self.trace("response-read", command, 0);
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

#[cfg(test)]
mod tests {
    //! Owns bounded retries and diagnostic ordering around the blocking transport.
    use super::*;
    use crate::{Operation, RequestId, TileId};
    use std::net::TcpListener;

    #[test]
    fn caller_deadline_caps_every_retry_and_phase_without_resetting() {
        let start = Instant::now();
        let end = start + Duration::from_millis(50);
        let budget = RetryBudget::new(start, Duration::from_secs(3), 3)
            .unwrap()
            .limit(end);
        assert_eq!(budget.phase_deadline(start).unwrap(), end);
        let now = std::cell::Cell::new(start);
        let attempts = std::cell::Cell::new(0);
        let result: io::Result<()> = exchanges(
            3,
            budget,
            || now.get(),
            |_| {
                attempts.set(attempts.get() + 1);
                now.set(end);
                Err(io::ErrorKind::WouldBlock.into())
            },
        );
        assert_eq!(attempts.get(), 1);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn expired_caller_deadline_sends_nothing_even_with_long_default_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let client =
            Client::new(listener.local_addr().unwrap(), Duration::from_secs(3), 3).unwrap();
        let result = client.call_before(
            &Command {
                id: RequestId(1),
                operation: Operation::Directory,
            },
            Instant::now(),
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn overall_deadline_prevents_a_second_exchange_at_27_seconds() {
        let start = Instant::now();
        let budget = RetryBudget::new(start, Duration::from_secs(3), 3).unwrap();
        assert_eq!(
            budget.deadline.duration_since(start),
            Duration::from_secs(27)
        );
        let clock = std::cell::Cell::new(start);
        let sent = std::cell::Cell::new(0);
        let result: io::Result<()> = exchanges(
            3,
            budget,
            || clock.get(),
            |_| {
                sent.set(sent.get() + 1);
                // The first uncertain exchange was paused until the absolute limit.
                clock.set(start + Duration::from_secs(27));
                Err(io::ErrorKind::ConnectionReset.into())
            },
        );
        assert_eq!(sent.get(), 1);
        let error = result.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(error.to_string().contains("result unknown"));
    }

    #[test]
    fn an_expired_call_waiting_for_the_handle_never_connects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let client =
            Client::new(listener.local_addr().unwrap(), Duration::from_millis(10), 1).unwrap();
        let held = client.stream.lock().unwrap();
        let result = client.call(&Command {
            id: RequestId(1),
            operation: Operation::Directory,
        });
        drop(held);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn final_phase_uses_only_the_remaining_overall_budget() {
        let start = Instant::now();
        let budget = RetryBudget::new(start, Duration::from_secs(3), 3).unwrap();
        let late = start + Duration::from_millis(26_999);
        assert_eq!(budget.phase_deadline(late).unwrap(), budget.deadline);
        assert!(budget.phase_deadline(budget.deadline).is_err());
    }

    #[test]
    fn expired_absolute_write_deadline_sends_no_frame_bytes() {
        use std::io::Read;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut writer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut reader, _) = listener.accept().unwrap();
        reader.set_nonblocking(true).unwrap();
        let command = Command {
            id: RequestId(1),
            operation: Operation::Directory,
        };
        let error = crate::codec::write_request_before(
            &mut writer,
            &command,
            Instant::now() - Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            reader.read(&mut [0u8; 1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn trace_attributes_the_lock_and_wire_to_the_original_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client =
            Client::new(listener.local_addr().unwrap(), Duration::from_secs(1), 1).unwrap();
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../.local/runs/e24-client-trace-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("trace.csv");
        client
            .trace_calls_to(File::create_new(&path).unwrap())
            .unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let received = crate::read_request(&mut stream, Duration::from_secs(1)).unwrap();
            assert_eq!(received.id, RequestId(123));
            crate::write_response(&mut stream, &Response::Ok, Duration::from_secs(1)).unwrap();
        });
        let command = Command {
            id: RequestId(123),
            operation: Operation::ClaimLease {
                tile: TileId(0),
                lease_ms: 1000,
            },
        };
        assert_eq!(client.call(&command).unwrap(), Response::Ok);
        worker.join().unwrap();
        let rows = std::fs::read_to_string(&path).unwrap();
        let events: Vec<_> = rows
            .lines()
            .skip(1)
            .map(|line| {
                let fields: Vec<_> = line.split(',').collect();
                assert_eq!(&fields[2..4], &["123", "claim-lease"]);
                fields[1]
            })
            .collect();
        assert_eq!(
            events,
            [
                "call-waiting",
                "lock-acquired",
                "attempt-start",
                "request-written",
                "response-read",
                "lock-releasing"
            ]
        );
        drop(client);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
