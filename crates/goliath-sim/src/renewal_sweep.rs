//! Owns thread-parallel CSV aggregation for store-crash renewal sweeps.

use goliath_sim::{RenewalFault, RenewalOutcome, RenewalPath, RenewalSetup, Stall, renewal_crash};
use std::error::Error;
use std::io::Write;

const LEASES: [u64; 3] = [500, 1000, 2000];

#[derive(Clone, Default)]
struct Totals {
    failures: u64,
    first_failure: Option<u64>,
    sum: RenewalOutcome,
}

impl Totals {
    fn add(&mut self, seed: u64, outcome: &RenewalOutcome) {
        if outcome.violated() {
            self.failures += 1;
            self.first_failure = Some(self.first_failure.map_or(seed, |first| first.min(seed)));
        }
        let sum = &mut self.sum;
        sum.overlapping_owners += outcome.overlapping_owners;
        sum.stale_writes += outcome.stale_writes;
        sum.beyond_store_window += outcome.beyond_store_window;
        sum.actions += outcome.actions;
        sum.claims += outcome.claims;
        sum.holds += outcome.holds;
        sum.late_claims += outcome.late_claims;
        sum.renewals += outcome.renewals;
        sum.unjournaled_grants += outcome.unjournaled_grants;
        sum.store_crashes += outcome.store_crashes;
        sum.renewals_lost += outcome.renewals_lost;
        sum.expiries += outcome.expiries;
        sum.stall_stops += outcome.stall_stops;
        sum.late_start_stops += outcome.late_start_stops;
        sum.longest_wait_ms = sum.longest_wait_ms.max(outcome.longest_wait_ms);
    }

    fn merge(mut self, other: Self) -> Self {
        let failures = self.failures + other.failures;
        let first = match (self.first_failure, other.first_failure) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        };
        self.add(0, &other.sum);
        self.failures = failures;
        self.first_failure = first;
        self
    }
}

// The binary owns these workers. Totals are sums, a minimum and a maximum, so
// the result does not depend on how seeds are divided.
fn run(seeds: u64, setup: RenewalSetup) -> Totals {
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers as u64)
            .map(|worker| {
                scope.spawn(move || {
                    let mut totals = Totals::default();
                    for seed in (worker..seeds).step_by(workers) {
                        totals.add(seed, &renewal_crash(seed, setup));
                    }
                    totals
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("sweep worker"))
            .fold(Totals::default(), Totals::merge)
    })
}

pub(crate) fn parse(
    path: &str,
    fault: &str,
    stall: &str,
) -> Option<(RenewalPath, RenewalFault, Stall)> {
    let path = match path {
        "barrier" => RenewalPath::Barrier,
        "unjournaled" => RenewalPath::Unjournaled,
        _ => return None,
    };
    let fault = std::iter::once(RenewalFault::None)
        .chain(RenewalFault::PLANTED)
        .find(|candidate| candidate.name() == fault)?;
    let stall = Stall::ALL
        .into_iter()
        .find(|candidate| candidate.name() == stall)?;
    Some((path, fault, stall))
}

pub(crate) fn sweep(seeds: u64, output: &str) -> Result<(), Box<dyn Error>> {
    if seeds == 0 {
        return Err("seeds must be nonzero".into());
    }
    let path = std::path::Path::new(output);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "path,fault,lease_ms,margin_ms,stall,seeds,failures,first_failure,owner_overlap,stale_writes,beyond_store_window,actions,claims,holds,late_claims,renewals,unjournaled_grants,store_crashes,renewals_lost,expiries,stall_stops,late_start_stops,longest_wait_ms"
    )?;
    let mut modes = vec![
        (RenewalPath::Barrier, RenewalFault::None),
        (RenewalPath::Unjournaled, RenewalFault::None),
    ];
    modes.extend(RenewalFault::PLANTED.map(|fault| (RenewalPath::Unjournaled, fault)));
    let mut problems = Vec::new();
    for lease_ms in LEASES {
        for (path, fault) in &modes {
            let mut caught = 0;
            for stall in Stall::ALL {
                let setup = RenewalSetup {
                    lease_ms,
                    stall,
                    path: *path,
                    fault: *fault,
                };
                let totals = run(seeds, setup);
                let label = match path {
                    RenewalPath::Barrier => "barrier",
                    RenewalPath::Unjournaled => "unjournaled",
                };
                let sum = &totals.sum;
                let first = totals
                    .first_failure
                    .map_or_else(|| "-".to_owned(), |seed| seed.to_string());
                writeln!(
                    file,
                    "{label},{},{lease_ms},{},{},{seeds},{},{first},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    fault.name(),
                    lease_ms / 10,
                    stall.name(),
                    totals.failures,
                    sum.overlapping_owners,
                    sum.stale_writes,
                    sum.beyond_store_window,
                    sum.actions,
                    sum.claims,
                    sum.holds,
                    sum.late_claims,
                    sum.renewals,
                    sum.unjournaled_grants,
                    sum.store_crashes,
                    sum.renewals_lost,
                    sum.expiries,
                    sum.stall_stops,
                    sum.late_start_stops,
                    sum.longest_wait_ms
                )?;
                println!(
                    "{label} {} L={lease_ms} {} seeds={seeds} failures={} stall_stops={}",
                    fault.name(),
                    stall.name(),
                    totals.failures,
                    sum.stall_stops
                );
                caught += totals.failures;
                if *fault == RenewalFault::None && totals.failures != 0 {
                    problems.push(format!(
                        "{label} L={lease_ms} {}: {} unsafe seeds, first {first}",
                        stall.name(),
                        totals.failures
                    ));
                }
                if *fault == RenewalFault::None
                    && *path == RenewalPath::Unjournaled
                    && sum.stall_stops != 0
                {
                    problems.push(format!(
                        "{label} L={lease_ms} {}: {} renewing holders stopped by a stalled barrier",
                        stall.name(),
                        sum.stall_stops
                    ));
                }
            }
            if *fault != RenewalFault::None && caught == 0 {
                problems.push(format!(
                    "{} L={lease_ms}: planted bug not caught",
                    fault.name()
                ));
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; ").into())
    }
}
