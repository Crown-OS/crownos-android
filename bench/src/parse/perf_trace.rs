use std::collections::HashSet;
use std::io::BufRead;

use serde::Serialize;

/// `sched:sched_wakeup` events counted from `perf script -F trace`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct WakeupTally {
    pub total: u64,
    /// Events whose woken thread is in the tracked set.
    pub tracked: u64,
}

/// The woken thread of a line such as `comm=surfaceflinger pid=812 prio=98 target_cpu=003`.
pub fn wakee(line: &[u8]) -> Option<u32> {
    let digits = line
        .split(u8::is_ascii_whitespace)
        .find_map(|field| field.strip_prefix(b"pid="))?;
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// Streams the trace, so a minute of system-wide events never sits in memory.
pub fn tally(trace: impl BufRead, threads: &HashSet<u32>) -> std::io::Result<WakeupTally> {
    let mut tally = WakeupTally::default();
    for line in trace.split(b'\n') {
        if let Some(thread) = wakee(&line?) {
            tally.total += 1;
            tally.tracked += u64::from(threads.contains(&thread));
        }
    }
    Ok(tally)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_wakeups_of_tracked_threads() -> std::io::Result<()> {
        let android = HashSet::from([812, 1093]);
        let trace: &[u8] = include_bytes!("../../fixtures/perf-script");
        assert_eq!(
            tally(trace, &android)?,
            WakeupTally {
                total: 5,
                tracked: 3
            }
        );
        assert_eq!(wakee(b"comm=init target_pid=1"), None);
        Ok(())
    }
}
