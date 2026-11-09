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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
            "start_us,count,admission_max_ms,queue_max_ms,queue_mean_ms,collect_ms,execute_ms,barrier_ms,dispatch_ms,maintain_ms,background_ms,mutations,accepted_sequence,receipt_count"
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
    let wall_before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let origin = Instant::now();
    let wall_after = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let mut request_trace = std::env::var_os("GOLIATH_STORE_TRACE")
        .map(|path| File::create(PathBuf::from(path).with_extension("requests.csv")))
        .transpose()?;
    let mut memory_trace = std::env::var_os("GOLIATH_STORE_TRACE")
        .map(|path| File::create(PathBuf::from(path).with_extension("memory.csv")))
        .transpose()?;
    if let Some(file) = &mut request_trace {
        writeln!(
            file,
            "request_id,batch_id,accepted_ns,received_ns,batch_start_ns,batch_end_ns,barrier_start_ns,barrier_end_ns,dispatched_ns,mutations"
        )?;
    }
    if let Some(file) = &mut memory_trace {
        writeln!(
            file,
            "at_ns,event,accepted,receipts,receipt_blob_bytes,receipt_blob_capacity_bytes,receipt_inline_bytes,players,uuid_players,player_blob_bytes,player_blob_capacity_bytes,snapshot_len,snapshot_capacity,in_flight,window_full_at_ns,snapshots_started,snapshots_finished"
        )?;
    }
    if let Some(path) = std::env::var_os("GOLIATH_STORE_TRACE") {
        let mut file = File::create(PathBuf::from(path).with_extension("origin.json"))?;
        writeln!(
            file,
            "{{\"native_instant\":\"{origin:?}\",\"wall_before_ns\":{wall_before},\"wall_after_ns\":{wall_after}}}"
        )?;
    }
    // Bounded queue and batch cap bound speculative memory. The binary owns
    // this worker and every connection thread, not the persistence library.
    let (sender, receiver) = mpsc::sync_channel::<Pending>(1024);
    std::thread::spawn(move || {
        let mut compaction: Option<std::thread::JoinHandle<(CompactionReady, Duration)>> = None;
        let mut snapshot_usage = None;
        let mut last_memory = origin;
        let mut filled = None;
        let mut batch_id = 0u64;
        let (mut snapshots_started, mut snapshots_finished) = (0u64, 0u64);
        while let Ok(first) = receiver.recv() {
            batch_id += 1;
            let mut snapshot_event = None;
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
            let explicit_compact = commands.iter().any(|c| c.operation == Operation::Compact);
            let mut background = Duration::ZERO;
            if (compaction.as_ref().is_some_and(|work| work.is_finished()) || explicit_compact)
                && let Some(work) = compaction.take()
            {
                let (ready, elapsed) = work.join().expect("compaction worker panicked");
                background = elapsed;
                engine
                    .complete_maintenance(Ok(ready))
                    .expect("compaction completion");
                snapshot_usage = None;
                snapshots_finished += 1;
                snapshot_event = Some("snapshot-finished");
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
            let accepted_before = engine.accepted_count();
            if explicit_compact {
                snapshots_started += 1;
            }
            let responses = match engine.execute_batch(&commands) {
                Ok(responses) => responses,
                Err(error) => {
                    eprintln!("fatal store IO: {error}");
                    std::process::exit(1);
                }
            };
            let execute_end = Instant::now();
            if explicit_compact {
                snapshots_finished += 1;
                snapshot_event = Some("snapshot-explicit");
            }
            let execute = execute_end.duration_since(execute_start);
            let mutations = engine.accepted_count() - accepted_before;
            let barrier = engine.last_barrier_duration();
            let barrier_span = engine.last_barrier_span();
            let dispatch_start = Instant::now();
            let mut request_rows = Vec::new();
            for (pending, response) in batch.into_iter().zip(responses) {
                let dispatched = Instant::now();
                if request_trace.is_some() {
                    request_rows.push((
                        pending.command.id.0,
                        pending.accepted,
                        pending.queued,
                        dispatched,
                    ));
                }
                let _ = pending.reply.send((response, dispatched));
            }
            let dispatch = dispatch_start.elapsed();
            let maintain_start = Instant::now();
            match engine.begin_maintenance() {
                Ok(Some(work)) => {
                    snapshot_usage = Some(work.snapshot_usage());
                    snapshots_started += 1;
                    snapshot_event = Some("snapshot-started");
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
                    "{},{count},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{mutations},{},{}",
                    execute_start.duration_since(origin).as_micros(),
                    ms(admission),
                    ms(queue),
                    queue_mean * 1000.0,
                    ms(execute_start.duration_since(collect_start)),
                    ms(execute),
                    ms(barrier),
                    ms(dispatch),
                    ms(maintain_start.elapsed()),
                    ms(background),
                    engine.accepted_count(),
                    engine.receipt_count()
                );
            }
            if let Some(file) = &mut request_trace {
                let at = |time: Instant| time.duration_since(origin).as_nanos();
                let span = barrier_span
                    .map(|(a, b)| (at(a) as i128, at(b) as i128))
                    .unwrap_or((-1, -1));
                for (id, accepted, received, dispatched) in request_rows {
                    let _ = writeln!(
                        file,
                        "{id},{batch_id},{},{},{},{},{},{},{},{mutations}",
                        at(accepted),
                        at(received),
                        at(execute_start),
                        at(execute_end),
                        span.0,
                        span.1,
                        at(dispatched)
                    );
                }
            }
            let first_full =
                filled.is_none() && engine.receipt_count() == goliath_store::RECEIPT_CAPACITY;
            if first_full {
                filled = Some(Instant::now());
            }
            if first_full
                || snapshot_event.is_some()
                || last_memory.elapsed() >= Duration::from_secs(1)
            {
                if let Some(file) = &mut memory_trace {
                    let stats = engine.resources();
                    let usage =
                        snapshot_usage.unwrap_or((stats.snapshot_len, stats.snapshot_capacity));
                    let now = Instant::now();
                    let full = filled
                        .map(|time| time.duration_since(origin).as_nanos() as i128)
                        .unwrap_or(-1);
                    let event = if first_full {
                        "window-full"
                    } else {
                        snapshot_event.unwrap_or("sample")
                    };
                    let _ = writeln!(
                        file,
                        "{},{event},{},{},{},{},{},{},{},{},{},{},{},{},{},{snapshots_started},{snapshots_finished}",
                        now.duration_since(origin).as_nanos(),
                        stats.accepted,
                        stats.receipts,
                        stats.receipt_blob_bytes,
                        stats.receipt_blob_capacity_bytes,
                        stats.receipt_inline_bytes,
                        stats.players,
                        stats.uuid_players,
                        stats.player_blob_bytes,
                        stats.player_blob_capacity_bytes,
                        usage.0,
                        usage.1,
                        u8::from(stats.in_flight),
                        full
                    );
                }
                last_memory = Instant::now();
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
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
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
