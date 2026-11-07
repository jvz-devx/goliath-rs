//! Owns independent worker processes and deterministic metric aggregation.

use goliath_model::Policy;
use goliath_sim::{Mutation, Outcome, SETUPS, Scenario, Settings, run_with_settings};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Debug, Default)]
struct Totals {
    seeds: u64,
    failures: u64,
    duplicates: u64,
    losses: u64,
    recovery_losses: u64,
    other_losses: u64,
    rollback: u64,
    owners: u64,
    stale: u64,
    completed: u64,
    aborted: u64,
    censored: u64,
    rejected_inputs: u64,
    embargo_crossings: u64,
    gaps: BTreeMap<u64, u64>,
    waits: BTreeMap<u64, u64>,
    first_failure: Option<u64>,
}

impl Totals {
    fn add(&mut self, seed: u64, outcome: Outcome, policy: Policy) {
        self.seeds += 1;
        let failed = outcome.violated() || (policy == Policy::P5 && outcome.losses > 0);
        self.failures += u64::from(failed);
        if failed {
            self.first_failure = Some(self.first_failure.map_or(seed, |s| s.min(seed)));
        }
        self.duplicates += outcome.duplicates;
        self.losses += outcome.losses;
        self.recovery_losses += outcome.loss_recovery;
        self.other_losses += outcome.loss_other;
        self.rollback += outcome.rollback;
        self.owners += outcome.owner_violations;
        self.stale += outcome.stale_writes;
        self.completed += outcome.completed;
        self.aborted += outcome.aborted;
        self.censored += outcome.censored;
        self.rejected_inputs += outcome.rejected_inputs;
        self.embargo_crossings += outcome.embargo_crossings;
        for gap in outcome.gaps {
            *self.gaps.entry(gap).or_default() += 1;
        }
        for wait in outcome.action_wait {
            *self.waits.entry(wait).or_default() += 1;
        }
    }

    fn merge(&mut self, other: Self) {
        self.seeds += other.seeds;
        self.failures += other.failures;
        self.duplicates += other.duplicates;
        self.losses += other.losses;
        self.recovery_losses += other.recovery_losses;
        self.other_losses += other.other_losses;
        self.rollback += other.rollback;
        self.owners += other.owners;
        self.stale += other.stale;
        self.completed += other.completed;
        self.aborted += other.aborted;
        self.censored += other.censored;
        self.rejected_inputs += other.rejected_inputs;
        self.embargo_crossings += other.embargo_crossings;
        if let Some(seed) = other.first_failure {
            self.first_failure = Some(self.first_failure.map_or(seed, |s| s.min(seed)));
        }
        for (value, count) in other.gaps {
            *self.gaps.entry(value).or_default() += count;
        }
        for (value, count) in other.waits {
            *self.waits.entry(value).or_default() += count;
        }
    }

    fn encode(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.seeds,
            self.failures,
            self.duplicates,
            self.losses,
            self.recovery_losses,
            self.other_losses,
            self.rollback,
            self.owners,
            self.stale,
            self.completed,
            self.aborted,
            self.censored,
            self.first_failure
                .map_or_else(|| "-".into(), |s| s.to_string()),
            histogram(&self.gaps),
            histogram(&self.waits),
            self.rejected_inputs,
            self.embargo_crossings,
            "v2"
        )
    }

    fn decode(value: &str) -> Result<Self, Box<dyn Error>> {
        let fields: Vec<&str> = value.split(',').collect();
        if fields.len() != 18 || fields[17] != "v2" {
            return Err("invalid worker row".into());
        }
        Ok(Self {
            seeds: fields[0].parse()?,
            failures: fields[1].parse()?,
            duplicates: fields[2].parse()?,
            losses: fields[3].parse()?,
            recovery_losses: fields[4].parse()?,
            other_losses: fields[5].parse()?,
            rollback: fields[6].parse()?,
            owners: fields[7].parse()?,
            stale: fields[8].parse()?,
            completed: fields[9].parse()?,
            aborted: fields[10].parse()?,
            censored: fields[11].parse()?,
            first_failure: if fields[12] == "-" {
                None
            } else {
                Some(fields[12].parse()?)
            },
            gaps: parse_histogram(fields[13])?,
            waits: parse_histogram(fields[14])?,
            rejected_inputs: fields[15].parse()?,
            embargo_crossings: fields[16].parse()?,
        })
    }
}

fn histogram(map: &BTreeMap<u64, u64>) -> String {
    map.iter()
        .map(|(v, n)| format!("{v}:{n}"))
        .collect::<Vec<_>>()
        .join(";")
}

fn parse_histogram(value: &str) -> Result<BTreeMap<u64, u64>, Box<dyn Error>> {
    let mut result = BTreeMap::new();
    for pair in value.split(';').filter(|p| !p.is_empty()) {
        let (value, count) = pair.split_once(':').ok_or("invalid histogram")?;
        result.insert(value.parse()?, count.parse()?);
    }
    Ok(result)
}

fn percentile(histogram: &BTreeMap<u64, u64>, p: u64) -> u64 {
    let rank = (histogram.values().sum::<u64>() * p).div_ceil(100);
    let mut count = 0;
    for (&value, &n) in histogram {
        count += n;
        if count >= rank {
            return value;
        }
    }
    0
}

pub(super) fn worker(
    index: u64,
    workers: u64,
    count: u64,
    calm: bool,
    abort_delay_ms: u64,
) -> Result<(), Box<dyn Error>> {
    if workers == 0 {
        return Err("worker count must be positive".into());
    }
    for (setting, latency) in SETUPS.into_iter().enumerate() {
        for policy in Policy::ALL {
            let mut totals = Totals::default();
            for seed in (index..count).step_by(workers as usize) {
                let mut scenario = Scenario::seeded(seed, latency);
                if calm {
                    scenario.faults.clear();
                }
                totals.add(
                    seed,
                    run_with_settings(
                        &scenario,
                        policy,
                        false,
                        Settings {
                            abort_delay_ms,
                            ..Settings::default()
                        },
                    ),
                    policy,
                );
            }
            println!("{setting},{}:{}", policy.name(), totals.encode());
        }
    }
    Ok(())
}

pub(super) fn sweep(
    count: u64,
    path: &str,
    calm: bool,
    abort_delay_ms: u64,
) -> Result<(), Box<dyn Error>> {
    if count == 0 {
        return Err("seed count must be positive".into());
    }
    let workers = std::thread::available_parallelism()?.get() as u64;
    eprintln!(
        "{} scenario-seeds, {workers} independent OS processes; assumed latencies",
        count * 18
    );
    let executable = std::env::current_exe()?;
    let mut children = Vec::new();
    for index in 0..workers {
        children.push(
            Command::new(&executable)
                .args([
                    "worker",
                    &index.to_string(),
                    &workers.to_string(),
                    &count.to_string(),
                    if calm { "calm" } else { "faults" },
                    &abort_delay_ms.to_string(),
                ])
                .stdout(Stdio::piped())
                .spawn()?,
        );
    }
    let mut totals: Vec<Vec<Totals>> = (0..3)
        .map(|_| (0..6).map(|_| Totals::default()).collect())
        .collect();
    for child in children {
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(format!("worker failed: {}", output.status).into());
        }
        for line in std::str::from_utf8(&output.stdout)?.lines() {
            let (key, value) = line.split_once(':').ok_or("invalid worker output")?;
            let (setting, policy) = key.split_once(',').ok_or("invalid worker key")?;
            let setting: usize = setting.parse()?;
            let policy = Policy::ALL
                .iter()
                .position(|p| p.name() == policy)
                .ok_or("invalid policy")?;
            totals[setting][policy].merge(Totals::decode(value)?);
        }
    }
    let mut csv = String::from(
        "policy,checkpoint_ms_assumed,fsync_ms_assumed,seeds,failing_seeds,duplicate_ids,loss_incidents,recovery_losses,other_losses,rollback_slots,owner_violations,stale_writes,completed,aborted,censored,gap_p50_ms,gap_p95_ms,gap_p99_ms,action_wait_p95_ms,first_failing_seed,rejected_inputs,embargo_crossings\n",
    );
    for (setting, latency) in SETUPS.iter().enumerate() {
        for (policy, total) in Policy::ALL.into_iter().zip(&totals[setting]) {
            writeln!(
                &mut csv,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                policy.name(),
                latency.checkpoint_ms,
                latency.fsync_ms,
                total.seeds,
                total.failures,
                total.duplicates,
                total.losses,
                total.recovery_losses,
                total.other_losses,
                total.rollback,
                total.owners,
                total.stale,
                total.completed,
                total.aborted,
                total.censored,
                percentile(&total.gaps, 50),
                percentile(&total.gaps, 95),
                percentile(&total.gaps, 99),
                percentile(&total.waits, 95),
                total
                    .first_failure
                    .map_or_else(|| "-".into(), |s| s.to_string()),
                total.rejected_inputs,
                total.embargo_crossings
            )?;
        }
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &csv)?;
    print!("{csv}");
    if !calm
        && totals
            .iter()
            .any(|row| row[0].failures == 0 || row[1].failures == 0)
    {
        return Err("control sweep did not expose both P0 and P1".into());
    }
    if totals
        .iter()
        .any(|row| row[2..].iter().any(|t| t.failures > 0 || t.losses > 0))
    {
        return Err("a candidate zero-duplication policy failed; replay the CSV seed".into());
    }
    Ok(())
}

pub(super) fn mutations(count: u64, path: &str) -> Result<(), Box<dyn Error>> {
    let mut csv = String::from("mutation,policy,setting,seeds,failing_seeds,first_seed\n");
    let mut escaped = false;
    for (mutation, policy) in Mutation::CASES {
        let mut caught = false;
        for (setting, latency) in SETUPS.into_iter().enumerate() {
            let mut failures = 0;
            let mut first = None;
            for seed in 0..count {
                let result = run_with_settings(
                    &Scenario::seeded(seed, latency),
                    policy,
                    false,
                    Settings {
                        mutation,
                        ..Settings::default()
                    },
                );
                if result.violated() || result.losses > 0 {
                    failures += 1;
                    first.get_or_insert(seed);
                }
            }
            caught |= failures > 0;
            writeln!(
                &mut csv,
                "{mutation:?},{},{setting},{count},{failures},{}",
                policy.name(),
                first.map_or_else(|| "-".into(), |s| s.to_string())
            )?;
        }
        escaped |= !caught;
    }
    std::fs::write(path, &csv)?;
    print!("{csv}");
    if escaped {
        return Err("a mutation escaped the invariant checker".into());
    }
    Ok(())
}
