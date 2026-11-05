use std::thread;
use std::time::{Duration, Instant};

use super::number::tenths;

const MILLISECONDS_PER_SECOND: f64 = 1000.0;

/// Milliseconds to a tenth, as results report them.
pub fn millis(duration: Duration) -> f64 {
    tenths(duration.as_secs_f64() * MILLISECONDS_PER_SECOND)
}

/// Polls `ready` every `interval` until it holds and returns how long after
/// `since` that was, or `None` once `timeout` after `since` has passed.
pub fn wait_until(
    since: Instant,
    timeout: Duration,
    interval: Duration,
    mut ready: impl FnMut() -> bool,
) -> Option<Duration> {
    loop {
        if ready() {
            return Some(since.elapsed());
        }
        if since.elapsed() >= timeout {
            return None;
        }
        thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_a_condition_or_gives_up() {
        let since = Instant::now();
        let mut polls = 0;
        let ready = wait_until(
            since,
            Duration::from_secs(5),
            Duration::from_millis(1),
            || {
                polls += 1;
                polls == 3
            },
        );
        assert!(ready.is_some_and(|elapsed| elapsed < Duration::from_secs(5)));
        let never = wait_until(
            Instant::now(),
            Duration::from_millis(20),
            Duration::from_millis(5),
            || false,
        );
        assert_eq!(never, None);
        assert_eq!(millis(Duration::from_micros(12_345)), 12.3);
    }
}
