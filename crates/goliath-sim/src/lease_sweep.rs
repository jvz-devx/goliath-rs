//! Owns compact, reproducible CSV aggregation for lease partition controls.

use goliath_sim::{LeaseMode, lease_partition};
use std::error::Error;
use std::io::Write;

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
        "mode,lease_ms,margin_ms,seeds,failures,stale_appends,stale_flushes,stale_installs,stale_logins,owner_overlap,duplicates,losses,rejected_actions,old_ticks_after_deadline,restarts,takeover_min_ms,takeover_max_ms"
    )?;
    for lease_ms in [500, 1000, 2000] {
        for mode in [LeaseMode::Enforce, LeaseMode::IgnoreDeadline] {
            let mut totals = [0_u64; 11];
            let mut min = u64::MAX;
            let mut max = 0;
            for seed in 0..seeds {
                let result = lease_partition(seed, lease_ms, mode);
                for (total, value) in totals.iter_mut().zip([
                    u64::from(result.violated()),
                    result.stale_appends,
                    result.stale_flushes,
                    result.stale_installs,
                    result.stale_logins,
                    result.overlapping_owners,
                    result.duplicate_identities,
                    result.missing_identities,
                    result.rejected_actions,
                    result.old_ticks_after_deadline,
                    u64::from(result.restarted),
                ]) {
                    *total += value;
                }
                min = min.min(result.takeover_ms);
                max = max.max(result.takeover_ms);
            }
            if (mode == LeaseMode::Enforce && totals[0] != 0)
                || (mode == LeaseMode::IgnoreDeadline && totals[0] != seeds)
            {
                return Err("lease safety or control failed".into());
            }
            let label = if mode == LeaseMode::Enforce {
                "leases"
            } else {
                "ignores-deadline"
            };
            write!(file, "{label},{lease_ms},{},{seeds}", lease_ms / 10)?;
            for total in totals {
                write!(file, ",{total}")?;
            }
            writeln!(file, ",{min},{max}")?;
            println!("{label} L={lease_ms} seeds={seeds} failures={}", totals[0]);
        }
    }
    Ok(())
}
