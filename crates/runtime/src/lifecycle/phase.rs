use std::time::Instant;

/// Where Android itself is, independent of whether it is installed.
#[derive(Clone, Debug)]
pub enum Phase {
    Stopped,
    /// Another user's session holds the container.
    Refused {
        message: String,
    },
    Booting(Boot),
    Running(Run),
    Stopping(AfterStop),
}

#[derive(Clone, Debug)]
pub struct Boot {
    /// No `Ready` by then triggers safe mode.
    pub deadline: Instant,
    pub stage: BootStage,
    pub safe_mode: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootStage {
    /// The host worker starts the container and session units.
    Session,
    /// Waiting for the agent socket to accept a connection.
    Agent,
    /// `Hello` sent, waiting for the agent's.
    Handshake,
    /// Handshake done, waiting for `Ready`.
    Ready,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub safe_mode: bool,
    pub link: Link,
    /// The warm-up boot waits this long for a Google services id.
    pub gsf_wait: Option<GsfWait>,
    /// Whether this run performed any errand, so the warm-up does not stop
    /// Android under a window that is about to open.
    pub served_errands: bool,
}

/// The agent connection of a running Android.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Up,
    /// `Hello` sent to a reconnected agent, waiting for its.
    Handshake,
    /// Handshake done; the agent refuses launches until it repeats `Ready`.
    AwaitReady,
    /// The agent went away; init restarts it, so wait before giving up.
    Lost {
        since: Instant,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct GsfWait {
    pub until: Instant,
    pub next_query: Instant,
}

/// What follows a stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterStop {
    Rest,
    /// Boot again, in safe mode when set.
    Restart {
        safe_mode: bool,
    },
}
