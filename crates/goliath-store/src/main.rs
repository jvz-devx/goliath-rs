//! Owns configuration, the localhost listener and all connection threads.

use goliath_store::{Command, Engine, Response, SyncMode, read_request, write_response};
use std::error::Error;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let mut directory = None;
    let mut address: SocketAddr = "127.0.0.1:0".parse()?;
    let mut mode = SyncMode::Full;
    let mut compact_every = 1024;
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
            #[cfg(feature = "fault-injection")]
            "--fault-disable-fence" => disable_fence = true,
            #[cfg(feature = "fault-injection")]
            "--fault-ack-before-barrier" => early_ack = true,
            #[cfg(feature = "fault-injection")]
            "--test-power-loss-layer" => power_loss_layer = true,
            _ => return Err("usage: goliath-store --dir PATH [--listen 127.0.0.1:0] [--sync full|fsync] [--compact-every 1024]".into()),
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
    run(engine, address)
}

struct Pending {
    command: Command,
    reply: mpsc::SyncSender<Response>,
}

fn run(mut engine: Engine, address: SocketAddr) -> Result<(), Box<dyn Error>> {
    // Bounded queue and batch cap bound speculative memory. The binary owns
    // this worker and every connection thread, not the persistence library.
    let (sender, receiver) = mpsc::sync_channel::<Pending>(1024);
    std::thread::spawn(move || {
        while let Ok(first) = receiver.recv() {
            let mut batch = vec![first];
            let deadline = Instant::now() + Duration::from_micros(500);
            while batch.len() < 256 {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(remaining) {
                    Ok(pending) => batch.push(pending),
                    Err(_) => break,
                }
            }
            let commands: Vec<_> = batch.iter().map(|p| p.command.clone()).collect();
            let responses = match engine.execute_batch(&commands) {
                Ok(responses) => responses,
                Err(error) => {
                    eprintln!("fatal store IO: {error}");
                    std::process::exit(1);
                }
            };
            for (pending, response) in batch.into_iter().zip(responses) {
                let _ = pending.reply.send(response);
            }
            if let Err(error) = engine.maintain() {
                eprintln!("fatal store compaction IO: {error}");
                std::process::exit(1);
            }
        }
    });
    let listener = TcpListener::bind(address)?;
    let (connections, streams) = mpsc::sync_channel::<TcpStream>(512);
    let streams = Arc::new(Mutex::new(streams));
    for _ in 0..64 {
        let streams = Arc::clone(&streams);
        let sender = sender.clone();
        std::thread::spawn(move || {
            loop {
                let stream = match streams.lock() {
                    Ok(receiver) => receiver.recv(),
                    Err(error) => {
                        eprintln!("fatal connection queue: {error}");
                        std::process::exit(1);
                    }
                };
                let Ok(stream) = stream else {
                    break;
                };
                if let Err(error) = connection(stream, &sender) {
                    eprintln!("connection: {error}");
                }
            }
        });
    }
    println!("LISTEN {}", listener.local_addr()?);
    io::stdout().flush()?;
    for stream in listener.incoming() {
        // A full admission queue closes the new socket without accepting work.
        // Callers retain an uncertain request ID and may retry it unchanged.
        let _ = connections.try_send(stream?);
    }
    Ok(())
}

fn connection(mut stream: TcpStream, sender: &mpsc::SyncSender<Pending>) -> io::Result<()> {
    let timeout = Duration::from_secs(5);
    stream.set_nodelay(true)?;
    let command = read_request(&mut stream, timeout)?;
    let (reply, response) = mpsc::sync_channel(1);
    sender
        .send(Pending { command, reply })
        .map_err(|_| io::Error::other("store worker stopped"))?;
    let response = response
        .recv()
        .map_err(|_| io::Error::other("store worker stopped"))?;
    write_response(&mut stream, &response, timeout)
}
