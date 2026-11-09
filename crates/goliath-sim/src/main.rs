//! Owns replay commands and parallel sweeps, never scenario scheduling.

mod lease_sweep;
mod renewal_sweep;
mod sweep;

use goliath_model::Policy;
use goliath_sim::{SETUPS, Scenario, run, shrink};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("leases") => {
            let seeds = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(20_000);
            let output = args.get(2).map(String::as_str).unwrap_or("docs/experiments/data/E16/leases.csv");
            lease_sweep::sweep(seeds, output)?;
        }
        Some("renewals") => {
            let seeds = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(20_000);
            let output = args.get(2).map(String::as_str).unwrap_or("docs/experiments/data/E29/renewals.csv");
            renewal_sweep::sweep(seeds, output)?;
        }
        Some("renewal") => {
            let [path, fault, lease_ms, stall, seed] = args.get(1..6).ok_or("expected path fault lease_ms stall seed")? else {
                return Err("expected path fault lease_ms stall seed".into());
            };
            let (path, fault, stall) = renewal_sweep::parse(path, fault, stall).ok_or("unknown path, fault or stall")?;
            let setup = goliath_sim::RenewalSetup { lease_ms: lease_ms.parse()?, stall, path, fault };
            let seed: u64 = seed.parse()?;
            let outcome = goliath_sim::renewal_crash(seed, setup);
            println!("seed={seed} violated={} {outcome:?}", outcome.violated());
        }
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
        _ => return Err("usage: goliath-sim sweep|calm [seeds=66667] [csv_path] [abort_delay_ms=40] | replay P0..P5 seed [setting=0] | mutations [seeds=20000] [csv_path] | leases [seeds=20000] [csv_path] | renewals [seeds=20000] [csv_path] | renewal barrier|unjournaled fault lease_ms calm|tail|severe seed".into()),
    }
    Ok(())
}
