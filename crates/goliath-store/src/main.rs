//! Owns configuration, the localhost listener and all connection threads.

use goliath_store::{
    Command, CompactionReady, Engine, Operation, Response, SyncMode, read_request, write_response,
};
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let mut directory = None;
    let mut address: SocketAddr = "127.0.0.1:0".parse()?;
    let mut mode = SyncMode::Full;
    let mut compact_every = 1024;
    let mut batch_us = 0;
    let mut args = std::env::args().skip(1);
    #[cfg(feature = "fault-injection")]
    let mut disable_fence = false;
    #[cfg(feature = "fault-injection")]
    let mut early_ack = false;
    #[cfg(feature = "fault-injection")]
    let mut power_loss_layer = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => directory = Some(PathBuf::from(args.next().ok_or("--dir needs a path")?)),
            "--listen" => address = args.next().ok_or("--listen needs an address")?.parse()?,
            "--sync" => mode = match args.next().as_deref() {
                Some("full") => SyncMode::Full, Some("fsync") => SyncMode::Fsync,
                _ => return Err("--sync needs full or fsync".into()),
            },
            "--compact-every" => compact_every = args.next().ok_or("--compact-every needs a count")?.parse()?,
            "--batch-us" => batch_us = args.next().ok_or("--batch-us needs microseconds")?.parse()?,
            #[cfg(feature = "fault-injection")]
            "--fault-disable-fence" => disable_fence = true,
            #[cfg(feature = "fault-injection")]
            "--fault-ack-before-barrier" => early_ack = true,
            #[cfg(feature = "fault-injection")]
            "--test-power-loss-layer" => power_loss_layer = true,
            _ => return Err("usage: goliath-store --dir PATH [--listen 127.0.0.1:0] [--sync full|fsync] [--compact-every 1024] [--batch-us 0]".into()),
        }
    }
    if !address.ip().is_loopback() {
        return Err("only loopback listeners are allowed".into());
    }
    let engine = Engine::open(&directory.ok_or("--dir is required")?, mode, compact_every)?;
    #[cfg(feature = "fault-injection")]
    let engine = {
        let mut engine = engine;
        if disable_fence {
            engine.disable_fence_for_test();
        }
        if early_ack {
            engine.acknowledge_before_barrier_for_test();
        }
        if power_loss_layer {
            engine.power_loss_file_layer_for_test();
        }
        engine
    };
    run(engine, address, Duration::from_micros(batch_us))
}

struct Pending {
    command: Command,
    reply: mpsc::SyncSender<(Response, Instant)>,
    queued: Instant,
    accepted: Instant,
}

struct ConnectionPermit(Arc<AtomicUsize>);

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

fn run(
    mut engine: Engine,
    address: SocketAddr,
    batch_window: Duration,
) -> Result<(), Box<dyn Error>> {
    let mut trace = std::env::var_os("GOLIATH_STORE_TRACE")
        .map(File::create)
        .transpose()?;
    if let Some(trace) = &mut trace {
        writeln!(
            trace,
            "start_us,count,admission_max_ms,queue_max_ms,queue_mean_ms,collect_ms,execute_ms,barrier_ms,dispatch_ms,maintain_ms,background_ms"
        )?;
    }
    let connection_trace = std::env::var_os("GOLIATH_STORE_TRACE")
        .map(|path| -> io::Result<_> {
            let mut file = File::create(PathBuf::from(path).with_extension("connections.csv"))?;
            writeln!(
                file,
                "request_id,commit,wake_ms,write_ms,admission_ms,total_ms"
            )?;
            Ok(Arc::new(Mutex::new(file)))
        })
        .transpose()?;
    let origin = Instant::now();
    // Bounded queue and batch cap bound speculative memory. The binary owns
    // this worker and every connection thread, not the persistence library.
    let (sender, receiver) = mpsc::sync_channel::<Pending>(1024);
    std::thread::spawn(move || {
        let mut compaction: Option<std::thread::JoinHandle<(CompactionReady, Duration)>> = None;
        while let Ok(first) = receiver.recv() {
            let mut batch = vec![first];
            let collect_start = Instant::now();
            let deadline = Instant::now() + batch_window;
            while batch.len() < 256 {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let next = if remaining.is_zero() {
                    receiver.try_recv().map_err(|_| ())
                } else {
                    receiver.recv_timeout(remaining).map_err(|_| ())
                };
                match next {
                    Ok(pending) => batch.push(pending),
                    Err(_) => break,
                }
            }
            let commands: Vec<_> = batch.iter().map(|p| p.command.clone()).collect();
            let mut background = Duration::ZERO;
            if (compaction.as_ref().is_some_and(|work| work.is_finished())
                || commands.iter().any(|c| c.operation == Operation::Compact))
                && let Some(work) = compaction.take()
            {
                let (ready, elapsed) = work.join().expect("compaction worker panicked");
                background = elapsed;
                engine
                    .complete_maintenance(Ok(ready))
                    .expect("compaction completion");
            }
            let execute_start = Instant::now();
            let count = batch.len();
            let admission = batch
                .iter()
                .map(|p| p.queued.duration_since(p.accepted))
                .max()
                .unwrap_or_default();
            let queue = batch
                .iter()
                .map(|p| execute_start.duration_since(p.queued))
                .max()
                .unwrap_or_default();
            let queue_mean = batch
                .iter()
                .map(|p| execute_start.duration_since(p.queued).as_secs_f64())
                .sum::<f64>()
                / count as f64;
            let responses = match engine.execute_batch(&commands) {
                Ok(responses) => responses,
                Err(error) => {
                    eprintln!("fatal store IO: {error}");
                    std::process::exit(1);
                }
            };
            let execute = execute_start.elapsed();
            let barrier = engine.last_barrier_duration();
            let dispatch_start = Instant::now();
            for (pending, response) in batch.into_iter().zip(responses) {
                let _ = pending.reply.send((response, Instant::now()));
            }
            let dispatch = dispatch_start.elapsed();
            let maintain_start = Instant::now();
            match engine.begin_maintenance() {
                Ok(Some(work)) => {
                    compaction = Some(std::thread::spawn(move || {
                        let begin = Instant::now();
                        let ready = work.run().unwrap_or_else(|error| {
                            eprintln!("fatal store compaction IO: {error}");
                            std::process::exit(1);
                        });
                        (ready, begin.elapsed())
                    }));
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("fatal store rotation IO: {error}");
                    std::process::exit(1);
                }
            }
            if let Some(trace) = &mut trace {
                let ms = |d: Duration| d.as_secs_f64() * 1000.0;
                let _ = writeln!(
                    trace,
                    "{},{count},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
                    execute_start.duration_since(origin).as_micros(),
                    ms(admission),
                    ms(queue),
                    queue_mean * 1000.0,
                    ms(execute_start.duration_since(collect_start)),
                    ms(execute),
                    ms(barrier),
                    ms(dispatch),
                    ms(maintain_start.elapsed()),
                    ms(background)
                );
            }
        }
    });
    let listener = TcpListener::bind(address)?;
    let active = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::new();
    println!("LISTEN {}", listener.local_addr()?);
    io::stdout().flush()?;
    for stream in listener.incoming() {
        let stream = stream?;
        let accepted = Instant::now();
        // Persistent clients must not occupy every slot of a smaller fixed pool.
        // Bound live sockets and their binary-owned workers together instead.
        if active
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count < 512).then_some(count + 1)
            })
            .is_err()
        {
            continue;
        }
        let permit = ConnectionPermit(Arc::clone(&active));
        let sender = sender.clone();
        let trace = connection_trace.clone();
        workers.retain(|worker: &std::thread::JoinHandle<()>| !worker.is_finished());
        workers.push(std::thread::Builder::new().spawn(move || {
            let _permit = permit;
            if let Err(error) = connection(stream, accepted, &sender, trace.as_ref()) {
                eprintln!("connection: {error}");
            }
        })?);
    }
    Ok(())
}

fn connection(
    mut stream: TcpStream,
    accepted: Instant,
    sender: &mpsc::SyncSender<Pending>,
    trace: Option<&Arc<Mutex<File>>>,
) -> io::Result<()> {
    let timeout = Duration::from_secs(5);
    stream.set_nodelay(true)?;
    let mut first = true;
    loop {
        // Idle handles may wait indefinitely, bounded by the 512-socket limit.
        // Once the first frame byte arrives, the whole-frame deadline still applies.
        stream.set_read_timeout(None)?;
        if stream.peek(&mut [0_u8])? == 0 {
            return Ok(());
        }
        let admitted = if first { accepted } else { Instant::now() };
        first = false;
        exchange(&mut stream, admitted, sender, trace, timeout)?;
    }
}

fn exchange(
    stream: &mut TcpStream,
    accepted: Instant,
    sender: &mpsc::SyncSender<Pending>,
    trace: Option<&Arc<Mutex<File>>>,
    timeout: Duration,
) -> io::Result<()> {
    let command = read_request(stream, timeout)?;
    let id = command.id.0;
    let commit = matches!(
        command.operation,
        Operation::Commit { .. } | Operation::CommitUuid { .. }
    );
    let queued = Instant::now();
    let (reply, response) = mpsc::sync_channel(1);
    sender
        .send(Pending {
            command,
            reply,
            queued,
            accepted,
        })
        .map_err(|_| io::Error::other("store worker stopped"))?;
    let (response, dispatched) = response
        .recv()
        .map_err(|_| io::Error::other("store worker stopped"))?;
    let wake = dispatched.elapsed();
    let write_start = Instant::now();
    write_response(stream, &response, timeout)?;
    let write = write_start.elapsed();
    let total = accepted.elapsed();
    if let Some(trace) = trace {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut trace = trace
            .lock()
            .map_err(|_| io::Error::other("trace lock poisoned"))?;
        writeln!(
            trace,
            "{id},{commit},{:.6},{:.6},{:.6},{:.6}",
            ms(wake),
            ms(write),
            ms(queued.duration_since(accepted)),
            ms(total)
        )?;
    }
    Ok(())
}
