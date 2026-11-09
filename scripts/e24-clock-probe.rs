//! Owns a read-only Darwin clock comparison, without changing lease authority.

use std::io::{self, Write};
use std::time::{Duration, Instant};

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
    // SAFETY: Darwin clock_gettime writes one initialized, correctly aligned
    // timespec with the ABI above. Only documented Darwin clock IDs are passed.
    assert_eq!(unsafe { clock_gettime(clock, &mut value) }, 0);
    u64::try_from(value.seconds).expect("nonnegative clock") * 1_000_000_000
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
    let maximum = next.after.saturating_sub(old.before);
    let minimum = next.before.saturating_sub(old.after);
    let Some(elapsed) = next.value.checked_sub(old.value) else {
        return true;
    };
    let slack = maximum / 500 + 1000;
    elapsed > maximum.saturating_add(slack) || elapsed.saturating_add(slack) < minimum
}
fn field(name: &str, value: Read) -> String {
    format!(
        "\"{name}\":{{\"instant_before_ns\":{},\"value_ns\":{},\"instant_after_ns\":{}}}",
        value.before, value.value, value.after
    )
}

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .expect("duration seconds")
        .parse()
        .expect("integer duration");
    assert!((1..=1500).contains(&seconds));
    let origin = Instant::now();
    let mut previous: Option<[Read; 3]> = None;
    let (mut count, mut mono_rejected, mut raw_rejected, mut uptime_rejected) =
        (0u64, 0u64, 0u64, 0u64);
    let mut last_report = Instant::now();
    let mut lower = i128::MAX;
    let mut upper = i128::MIN;
    let mut maximum_raw_bracket = 0;
    let mut output = io::BufWriter::new(io::stdout().lock());
    while origin.elapsed() < Duration::from_secs(seconds) {
        // Darwin IDs from the installed SDK: MONOTONIC=6, MONOTONIC_RAW=4,
        // UPTIME_RAW=8. These are not portable Linux clock IDs.
        let mono = read(origin, 6);
        let uptime_before = native(8);
        let raw = read(origin, 4);
        let uptime_after = native(8);
        let uptime = read(origin, 8);
        let now = [mono, raw, uptime];
        lower = lower.min(i128::from(raw.value) - i128::from(uptime_after));
        upper = upper.max(i128::from(raw.value) - i128::from(uptime_before));
        maximum_raw_bracket = maximum_raw_bracket.max(uptime_after - uptime_before);
        let bad = previous
            .map(|old| {
                [
                    rejected(old[0], mono),
                    rejected(old[1], raw),
                    rejected(old[2], uptime),
                ]
            })
            .unwrap_or([false; 3]);
        mono_rejected += u64::from(bad[0]);
        raw_rejected += u64::from(bad[1]);
        uptime_rejected += u64::from(bad[2]);
        count += 1;
        if bad.iter().any(|value| *value) || last_report.elapsed() >= Duration::from_secs(1) {
            let wall = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix clock")
                .as_nanos();
            let old = previous.unwrap_or(now);
            writeln!(output,"{{\"kind\":\"{}\",\"wall_ns\":{wall},\"samples\":{count},\"elapsed_ns\":{},\"rejected\":[{},{},{}],\"reject_totals\":[{mono_rejected},{raw_rejected},{uptime_rejected}],\"raw_minus_uptime_lower_ns\":{lower},\"raw_minus_uptime_upper_ns\":{upper},\"maximum_raw_pair_bracket_ns\":{maximum_raw_bracket},\"previous\":{{{},{},{}}},\"next\":{{{},{},{}}}}}",
                if bad.iter().any(|value|*value) {"rejection"} else {"periodic"},origin.elapsed().as_nanos(),bad[0],bad[1],bad[2],
                field("monotonic",old[0]),field("monotonic_raw",old[1]),field("uptime_raw",old[2]),
                field("monotonic",now[0]),field("monotonic_raw",now[1]),field("uptime_raw",now[2])).expect("write evidence");
            output.flush().expect("flush evidence");
            last_report = Instant::now();
        }
        previous = Some(now);
    }
    writeln!(output,"{{\"kind\":\"complete\",\"samples\":{count},\"elapsed_ns\":{},\"reject_totals\":[{mono_rejected},{raw_rejected},{uptime_rejected}],\"raw_minus_uptime_lower_ns\":{lower},\"raw_minus_uptime_upper_ns\":{upper},\"maximum_raw_pair_bracket_ns\":{maximum_raw_bracket}}}",origin.elapsed().as_nanos()).expect("write completion");
}
