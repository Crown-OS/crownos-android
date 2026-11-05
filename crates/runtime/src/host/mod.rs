//! Thin adapters over the machine: systemd, Waydroid and logind over D-Bus,
//! the `waydroid` command, and the session data home. Everything here blocks,
//! so the daemon runs it on its host worker thread.

mod bus;
mod command;
pub mod data_home;
pub mod logind;
mod sequence;
pub mod systemd;
pub mod waydroid;

pub use bus::Buses;
pub use sequence::{SessionPlan, start_session, stop_session};

use std::time::Duration;

/// The system unit that runs Waydroid's container manager.
pub const CONTAINER_SERVICE: &str = "waydroid-container.service";
/// The user unit that runs `waydroid session start`.
pub const SESSION_SERVICE: &str = "crownos-android-session.service";
/// The system unit of the setup service, started again by "Try again".
pub const SETUP_SERVICE: &str = "crownos-android-setup.service";

/// How often a wait re-checks its condition.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("D-Bus: {0}")]
    Bus(#[from] zbus::Error),
    #[error("D-Bus value: {0}")]
    Value(#[from] zbus::zvariant::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{what} did not happen within {seconds} s")]
    Timeout { what: String, seconds: u64 },
    #[error("{unit} is {state}")]
    Unit { unit: String, state: String },
    #[error("`{command}` failed: {detail}")]
    Command { command: String, detail: String },
    #[error("the container runs a session of uid {uid}")]
    OtherUser { uid: u32 },
}

/// Polls `ready` until it returns `Some`, failing after `timeout`.
fn wait_for<T>(
    what: &str,
    timeout: Duration,
    mut ready: impl FnMut() -> Result<Option<T>, HostError>,
) -> Result<T, HostError> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(value) = ready()? {
            return Ok(value);
        }
        if std::time::Instant::now() >= deadline {
            return Err(HostError::Timeout {
                what: what.to_owned(),
                seconds: timeout.as_secs(),
            });
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
