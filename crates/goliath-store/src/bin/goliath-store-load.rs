//! Owns an independent threaded latency driver without Python's interpreter lock.

use goliath_model::{Fence, Generation, OwnershipCas, PlayerId, TileId};
use goliath_store::{Blob, CallTiming, Client, Command, Operation, RequestId, Response};
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::Barrier;
use std::time::{Duration, Instant};

fn blob(player: usize, generation: u64) -> Blob {
    let mut bytes = vec![0x5c; 256];
    bytes[..8].copy_from_slice(&(player as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&generation.to_le_bytes());
    Blob { version: 1, bytes }
}

fn commit(player: usize, generation: u64, fences: [Fence; 2]) -> Command {
    let source = generation as usize % 2;
    Command {
        id: RequestId((0xe9_u128 << 112) + ((player as u128) << 64) + u128::from(generation) + 1),
        operation: Operation::Commit {
            player: PlayerId(player),
            cas: OwnershipCas {
                generation: Generation(generation),
                source: fences[source],
                target: fences[1 - source],
            },
            blob: blob(player, generation + 1),
        },
    }
}

fn checked(client: &Client, command: &Command, generation: u64) -> io::Result<CallTiming> {
    let (response, timing) = client.call_with_timing(command)?;
    if response != Response::Committed(Generation(generation)) {
        return Err(io::Error::other(format!(
            "unexpected commit response {response:?}"
        )));
    }
    Ok(timing)
}

struct Sample {
    elapsed: Duration,
    timing: CallTiming,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let fresh = args.get(3).is_some_and(|arg| arg == "--fresh");
    if args.len() != 3 && !(args.len() == 4 && fresh) {
        return Err(
            "usage: goliath-store-load ADDRESS CLIENTS SAMPLES_PER_CLIENT [--fresh]".into(),
        );
    }
    let address: SocketAddr = args[0].parse()?;
    let clients: usize = args[1].parse()?;
    let samples: u64 = args[2].parse()?;
    if clients == 0 || clients > 256 || samples == 0 || samples > 1_000_000 {
        return Err("need 1..256 clients and 1..1000000 samples".into());
    }
    let client = Client::new(address, Duration::from_secs(5), 1)?;
    let Response::Directory(entries) = client.call(&Command {
        id: RequestId(0),
        operation: Operation::Directory,
    })?
    else {
        return Err("missing directory".into());
    };
    let mut fences = Vec::new();
    for tile in [TileId(0), TileId(1)] {
        fences.push(
            entries
                .iter()
                .find(|e| e.active && e.fence.tile == tile)
                .ok_or("missing tile fence")?
                .fence,
        );
    }
    let fences: [Fence; 2] = fences.try_into().map_err(|_| "need two fences")?;
    let make_client = || {
        if fresh {
            Client::fresh(address, Duration::from_secs(5), 1)
        } else {
            Client::new(address, Duration::from_secs(5), 1)
        }
    };
    let measured_clients = (0..clients)
        .map(|_| make_client())
        .collect::<io::Result<Vec<_>>>()?;
    for (player, client) in measured_clients.iter().enumerate() {
        checked(client, &commit(player, 0, fences), 1)?;
    }
    let start = Barrier::new(clients);
    let groups = std::thread::scope(|scope| -> io::Result<Vec<Vec<Sample>>> {
        let mut workers = Vec::new();
        for (player, client) in measured_clients.iter().enumerate() {
            let start = &start;
            workers.push(scope.spawn(move || -> io::Result<Vec<Sample>> {
                let mut values = Vec::new();
                start.wait();
                for index in 0..samples {
                    let command = commit(player, index + 1, fences);
                    let begin = Instant::now();
                    let timing = checked(client, &command, index + 2)?;
                    values.push(Sample {
                        elapsed: begin.elapsed(),
                        timing,
                    });
                }
                Ok(values)
            }));
        }
        workers
            .into_iter()
            .map(|w| {
                w.join()
                    .map_err(|_| io::Error::other("load worker panicked"))?
            })
            .collect()
    })?;
    for player in 0..clients {
        let Response::Player(Some(record)) = client.call(&Command {
            id: RequestId(0),
            operation: Operation::Read(PlayerId(player)),
        })?
        else {
            return Err("missing measured player".into());
        };
        if record.generation != Generation(samples + 1)
            || record.owner != TileId((samples as usize + 1) % 2)
            || record.inventory != blob(player, samples + 1)
        {
            return Err("measured player state mismatch".into());
        }
    }
    println!("player,index,milliseconds,connect_ms,write_ms,read_ms");
    for (player, group) in groups.iter().enumerate() {
        for (index, value) in group.iter().enumerate() {
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            println!(
                "{player},{index},{:.6},{:.6},{:.6},{:.6}",
                ms(value.elapsed),
                ms(value.timing.connect),
                ms(value.timing.write),
                ms(value.timing.read)
            );
        }
    }
    Ok(())
}
