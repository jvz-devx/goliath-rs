//! Owns exhaustive raw-pair interval and triple-width evidence, without authority.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}
unsafe extern "C" {
    fn clock_gettime(clock: u32, value: *mut Timespec) -> i32;
}

fn native(clock: u32) -> u64 {
    let mut value = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: The installed Darwin ABI uses this timespec layout. The pointer
    // is valid and aligned for one write. Only SDK clock IDs 4, 6, 8 are passed.
    assert_eq!(unsafe { clock_gettime(clock, &mut value) }, 0);
    u64::try_from(value.seconds).expect("nonnegative seconds") * 1_000_000_000
        + u64::try_from(value.nanoseconds).expect("nonnegative nanoseconds")
}

#[derive(Clone, Copy)]
struct Read {
    before: u64,
    value: u64,
    after: u64,
}
fn read(origin: Instant, clock: u32) -> Read {
    let before = origin.elapsed().as_nanos() as u64;
    let value = native(clock);
    let after = origin.elapsed().as_nanos() as u64;
    Read {
        before,
        value,
        after,
    }
}
fn rejected(old: Read, next: Read) -> bool {
    let maximum = next.after.checked_sub(old.before).expect("ordered Instant");
    let minimum = next.before.saturating_sub(old.after);
    let Some(elapsed) = next.value.checked_sub(old.value) else {
        return true;
    };
    let slack = maximum / 500 + 1000;
    elapsed > maximum + slack || elapsed + slack < minimum
}
fn field(name: &str, value: Read) -> String {
    format!(
        "\"{name}\":{{\"instant_before_ns\":{},\"value_ns\":{},\"instant_after_ns\":{}}}",
        value.before, value.value, value.after
    )
}

#[derive(Clone, Copy)]
struct Triple {
    uptime_before: u64,
    continuous: u64,
    uptime_after: u64,
}
impl Triple {
    fn width(self) -> u64 {
        self.uptime_after
            .checked_sub(self.uptime_before)
            .expect("uptime reversed")
    }
    fn interval(self) -> (i128, i128) {
        (
            i128::from(self.continuous) - i128::from(self.uptime_after),
            i128::from(self.continuous) - i128::from(self.uptime_before),
        )
    }
    fn json(self) -> String {
        format!(
            "{{\"uptime_before_ns\":{},\"continuous_ns\":{},\"uptime_after_ns\":{},\"width_ns\":{}}}",
            self.uptime_before,
            self.continuous,
            self.uptime_after,
            self.width()
        )
    }
}

struct Summary {
    widths: BTreeMap<u64, u64>,
    count: u64,
    lower: i128,
    upper: i128,
    narrowest: Option<Triple>,
    baseline: Option<Triple>,
    baseline_retakes: u64,
    wider_1000: u64,
    wider_2000: u64,
    run_wide: u64,
    longest_wide: u64,
    inconclusive: u64,
    run_inconclusive: u64,
    longest_inconclusive: u64,
    definite_offset: u64,
}
impl Summary {
    fn new() -> Self {
        Self {
            widths: BTreeMap::new(),
            count: 0,
            lower: i128::MIN,
            upper: i128::MAX,
            narrowest: None,
            baseline: None,
            baseline_retakes: 0,
            wider_1000: 0,
            wider_2000: 0,
            run_wide: 0,
            longest_wide: 0,
            inconclusive: 0,
            run_inconclusive: 0,
            longest_inconclusive: 0,
            definite_offset: 0,
        }
    }
    fn observe(&mut self, value: Triple) {
        let width = value.width();
        *self.widths.entry(width).or_default() += 1;
        self.count += 1;
        let (lower, upper) = value.interval();
        self.lower = self.lower.max(lower);
        self.upper = self.upper.min(upper);
        if self.narrowest.is_none_or(|old| width < old.width()) {
            self.narrowest = Some(value)
        }
        self.wider_1000 += u64::from(width > 1000);
        self.wider_2000 += u64::from(width > 2000);
        self.run_wide = if width > 2000 { self.run_wide + 1 } else { 0 };
        self.longest_wide = self.longest_wide.max(self.run_wide);
        // This is evidence for a proposed 500 ns initial-bracket limit and a
        // single 1000 ns claim-wide allowance. There is no lease or authority.
        if self.baseline.is_none() {
            if width <= 500 {
                self.baseline = Some(value)
            } else {
                self.baseline_retakes += 1
            }
        }
        if let Some(anchor) = self.baseline {
            let (anchor_lower, anchor_upper) = anchor.interval();
            let delta_lower = lower - anchor_upper;
            let delta_upper = upper - anchor_lower;
            let definite = delta_lower > 1000 || delta_upper < -1000;
            let uncertain = !definite && (delta_lower < -1000 || delta_upper > 1000);
            self.definite_offset += u64::from(definite);
            self.inconclusive += u64::from(uncertain);
            self.run_inconclusive = if uncertain {
                self.run_inconclusive + 1
            } else {
                0
            };
            self.longest_inconclusive = self.longest_inconclusive.max(self.run_inconclusive);
        }
    }
    fn quantile(&self, numerator: u64, denominator: u64) -> u64 {
        let rank = (self.count * numerator).div_ceil(denominator);
        let mut count = 0;
        for (&width, &amount) in &self.widths {
            count += amount;
            if count >= rank {
                return width;
            }
        }
        unreachable!("histogram covers every triple")
    }
    fn json(&self, full: bool) -> String {
        let histogram = if full {
            format!(
                ",\"width_histogram_ns_count\":[{}]",
                self.widths
                    .iter()
                    .map(|(w, n)| format!("[{w},{n}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        } else {
            String::new()
        };
        format!(
            "{{\"triples\":{},\"intersection_lower_ns\":{},\"intersection_upper_ns\":{},\"fixed_offset_is_consistent\":{},\"narrowest\":{},\"baseline\":{},\"baseline_width_limit_ns\":500,\"baseline_retakes\":{},\"wider_than_1000_ns\":{},\"wider_than_2000_ns\":{},\"longest_consecutive_wider_than_2000_ns\":{},\"fixed_baseline_inconclusive\":{},\"longest_consecutive_fixed_baseline_inconclusive\":{},\"fixed_baseline_definite_offset\":{},\"width_distribution_ns\":{{\"p50\":{},\"p99\":{},\"p99_99\":{},\"max\":{}}}{histogram}}}",
            self.count,
            self.lower,
            self.upper,
            self.lower <= self.upper,
            self.narrowest.expect("observed triple").json(),
            self.baseline.map(Triple::json).unwrap_or("null".into()),
            self.baseline_retakes,
            self.wider_1000,
            self.wider_2000,
            self.longest_wide,
            self.inconclusive,
            self.longest_inconclusive,
            self.definite_offset,
            self.quantile(50, 100),
            self.quantile(99, 100),
            self.quantile(9999, 10000),
            self.widths.last_key_value().expect("histogram").0
        )
    }
}

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .expect("seconds")
        .parse()
        .expect("integer seconds");
    assert!((1..=1500).contains(&seconds));
    let origin = Instant::now();
    let mut previous = None;
    let mut totals = [0u64; 3];
    let mut summary = Summary::new();
    let mut last_report = Instant::now();
    let mut was_consistent = true;
    let mut output = io::BufWriter::new(io::stdout().lock());
    loop {
        let mono = read(origin, 6);
        let before = origin.elapsed().as_nanos() as u64;
        let uptime_before = native(8);
        let continuous = native(4);
        let uptime_after = native(8);
        let after = origin.elapsed().as_nanos() as u64;
        let triple = Triple {
            uptime_before,
            continuous,
            uptime_after,
        };
        let raw = Read {
            before,
            value: continuous,
            after,
        };
        let uptime = read(origin, 8);
        let now = [mono, raw, uptime];
        summary.observe(triple);
        let bad = previous
            .map(|old: [Read; 3]| {
                [
                    rejected(old[0], mono),
                    rejected(old[1], raw),
                    rejected(old[2], uptime),
                ]
            })
            .unwrap_or([false; 3]);
        for (total, bad) in totals.iter_mut().zip(bad) {
            *total += u64::from(bad)
        }
        let complete = origin.elapsed() >= Duration::from_secs(seconds);
        let consistent = summary.lower <= summary.upper;
        if complete
            || bad.iter().any(|bad| *bad)
            || (was_consistent && !consistent)
            || last_report.elapsed() >= Duration::from_secs(1)
        {
            let wall = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix time")
                .as_nanos();
            let old = previous.unwrap_or(now);
            let kind = if complete {
                "complete"
            } else if !consistent && was_consistent {
                "raw-offset-intersection-empty"
            } else if bad.iter().any(|bad| *bad) {
                "rejection"
            } else {
                "periodic"
            };
            writeln!(output,"{{\"schema\":\"E24-exhaustive-clock-probe-v2\",\"kind\":\"{kind}\",\"wall_ns\":{wall},\"samples\":{},\"elapsed_ns\":{},\"rejected\":[{},{},{}],\"reject_totals\":[{},{},{}],\"raw_pair\":{},\"triple\":{},\"previous\":{{{},{},{}}},\"next\":{{{},{},{}}}}}",
                summary.count,origin.elapsed().as_nanos(),bad[0],bad[1],bad[2],totals[0],totals[1],totals[2],summary.json(complete),triple.json(),
                field("monotonic",old[0]),field("monotonic_raw",old[1]),field("uptime_raw",old[2]),field("monotonic",now[0]),field("monotonic_raw",now[1]),field("uptime_raw",now[2])).expect("write evidence");
            output.flush().expect("flush evidence");
            last_report = Instant::now();
        }
        previous = Some(now);
        was_consistent = consistent;
        if complete {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_triple_contributes_to_the_intersection_and_exact_histogram() {
        let mut summary = Summary::new();
        for (base, width) in [
            (0, 100),
            (10_000, 3000),
            (20_000, 4000),
            (30_000, 100),
            (40_000, 3000),
        ] {
            summary.observe(Triple {
                uptime_before: base,
                continuous: base + width / 2,
                uptime_after: base + width,
            });
        }
        assert_eq!(summary.count, 5);
        assert_eq!(summary.widths.values().sum::<u64>(), 5);
        assert_eq!(summary.quantile(50, 100), 3000);
        assert_eq!(summary.quantile(99, 100), 4000);
        assert_eq!(summary.quantile(9999, 10000), 4000);
        assert_eq!(summary.wider_1000, 3);
        assert_eq!(summary.wider_2000, 3);
        assert_eq!(summary.longest_wide, 2);
        assert_eq!(summary.longest_inconclusive, 2);
        assert!(summary.lower <= 0 && summary.upper >= 0);
        assert_eq!(summary.narrowest.expect("narrow triple").width(), 100);
    }

    #[test]
    fn a_short_lived_offset_change_cannot_hide_between_periodic_rows() {
        let mut summary = Summary::new();
        summary.observe(Triple {
            uptime_before: 0,
            continuous: 50,
            uptime_after: 100,
        });
        summary.observe(Triple {
            uptime_before: 1000,
            continuous: 22_050,
            uptime_after: 1100,
        });
        summary.observe(Triple {
            uptime_before: 2000,
            continuous: 2050,
            uptime_after: 2100,
        });
        assert!(summary.lower > summary.upper);
        assert_eq!(summary.definite_offset, 1);
        assert_eq!(summary.widths.values().sum::<u64>(), 3);
    }
}
