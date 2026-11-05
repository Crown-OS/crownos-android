use std::time::{Duration, Instant};

/// How often the agent is asked for keep-alive reasons while no window is open.
pub const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(60);
/// How long the last keep-alive question before a stop waits for its answer.
pub const KEEP_ALIVE_REPLY: Duration = Duration::from_secs(5);
const SECONDS_PER_MINUTE: u64 = 60;

/// What the idle timer wants next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleStep {
    Wait,
    /// Send `QueryKeepAlive`.
    Query,
    /// Stop Android.
    Stop,
}

/// Counts down from the moment the last window closed or the last keep-alive
/// reason went away.
#[derive(Clone, Debug)]
pub struct IdleTimer {
    timeout: Option<Duration>,
    since: Option<Instant>,
    next_query: Option<Instant>,
    answer_due: Option<Instant>,
}

impl IdleTimer {
    /// `minutes` 0 never stops Android.
    pub fn new(minutes: u32) -> Self {
        Self {
            timeout: timeout(minutes),
            since: None,
            next_query: None,
            answer_due: None,
        }
    }

    pub fn set_minutes(&mut self, minutes: u32, now: Instant) {
        self.timeout = timeout(minutes);
        if self.since.is_some() {
            self.arm(now);
        }
    }

    pub fn arm(&mut self, now: Instant) {
        self.since = Some(now);
        self.next_query = self.timeout.map(|_| now + KEEP_ALIVE_INTERVAL);
        self.answer_due = None;
    }

    /// Starts the countdown when no window is open and it is not running yet.
    pub fn arm_if_idle(&mut self, no_windows: bool, now: Instant) {
        if no_windows && !self.is_armed() {
            self.arm(now);
        }
    }

    pub fn disarm(&mut self) {
        self.since = None;
        self.next_query = None;
        self.answer_due = None;
    }

    pub fn is_armed(&self) -> bool {
        self.since.is_some()
    }

    pub fn deadline(&self) -> Option<Instant> {
        Some(self.since? + self.timeout?)
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        if self.answer_due.is_some() {
            return self.answer_due;
        }
        [self.deadline(), self.next_query]
            .into_iter()
            .flatten()
            .min()
    }

    pub fn poll(&mut self, now: Instant) -> IdleStep {
        if let Some(due) = self.answer_due {
            return if now >= due {
                IdleStep::Stop
            } else {
                IdleStep::Wait
            };
        }
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            self.answer_due = Some(now + KEEP_ALIVE_REPLY);
            return IdleStep::Query;
        }
        match self.next_query {
            Some(query) if now >= query => {
                self.next_query = Some(now + KEEP_ALIVE_INTERVAL);
                IdleStep::Query
            }
            _ => IdleStep::Wait,
        }
    }

    /// An answer to `QueryKeepAlive`. Any reason starts the countdown again.
    pub fn keep_alive(&mut self, any_reason: bool, now: Instant) -> IdleStep {
        if !self.is_armed() {
            return IdleStep::Wait;
        }
        if any_reason {
            self.arm(now);
            return IdleStep::Wait;
        }
        if self.answer_due.is_some() {
            IdleStep::Stop
        } else {
            IdleStep::Wait
        }
    }
}

fn timeout(minutes: u32) -> Option<Duration> {
    (minutes > 0).then(|| Duration::from_secs(u64::from(minutes) * SECONDS_PER_MINUTE))
}
