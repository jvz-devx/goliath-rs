//! Owns replay commands and process-parallel sweeps, never scenario scheduling.

mod sweep;

use goliath_model::Policy;
use goliath_sim::{SETUPS, Scenario, run, shrink};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("sweep" | "calm") => {
            let count = args.get(1).map(|v| v.parse()).transpose()?.unwrap_or(66_667);
            let output = args.get(2).map(String::as_str).unwrap_or("docs/experiments/data/E9/sweep.csv");
            let abort_delay = args.get(3).map(|v| v.parse()).transpose()?.unwrap_or(40);
            sweep::sweep(count, output, args[0] == "calm", abort_delay)?;
        }
        Some("worker") => {
            sweep::worker(args[1].parse()?, args[2].parse()?, args[3].parse()?, args.get(4).is_some_and(|s| s == "calm"), args[5].parse()?)?;
        }
        Some("mutations") => {
            let count = args.get(1).map(|v| v.parse()).transpose()?.unwrap_or(20_000);
            let output = args.get(2).map(String::as_str).unwrap_or("docs/experiments/data/E9/mutations.csv");
            sweep::mutations(count, output)?;
        }
        Some("replay") => {
            let policy = args.get(1).and_then(|s| Policy::parse(s)).ok_or("expected policy P0..P5")?;
            let seed = args.get(2).ok_or("expected seed")?.parse()?;
            let setting: usize = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(0);
            let latency = *SETUPS.get(setting).ok_or("setting must be 0, 1 or 2")?;
            let scenario = Scenario::seeded(seed, latency);
            let original = run(&scenario, policy, false);
            println!("original seed={seed} policy={} setting={setting} hash={:016x} duplicates={} losses={} owners={} stale={}", policy.name(), original.hash, original.duplicates, original.losses, original.owner_violations, original.stale_writes);
            let reduced = shrink(&scenario, policy);
            println!("1-minimal deletion schedule: faults={:?}, inputs={:?}", reduced.faults, reduced.inputs);
            let result = run(&reduced, policy, true);
            let failure = result.first_failure.as_ref().and_then(|s| s.split_once(": Audit")).map(|(description, _)| description);
            for line in &result.trace {
                println!("{line}");
                if failure.is_some_and(|description| line.starts_with(description)) { break; }
            }
            println!("reduced hash={:016x} first_failure={:?}", result.hash, result.first_failure);
        }
        _ => return Err("usage: goliath-sim sweep|calm [seeds=66667] [csv_path] [abort_delay_ms=40] | replay P0..P5 seed [setting=0] | mutations [seeds=20000] [csv_path]".into()),
    }
    Ok(())
}
