//! The daemon's Wayland connection, made at start so a splash appears at
//! once, and retried with backoff: at login the daemon may start before the
//! compositor has exported `WAYLAND_DISPLAY` to the user manager.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use runtime::host::Buses;
use runtime::host::systemd::{Systemd, lookup};
use splash::{Splashes, Tone};

const FIRST_RETRY: Duration = Duration::from_millis(250);
const MAX_RETRY: Duration = Duration::from_secs(10);
const WAYLAND_DISPLAY: &str = "WAYLAND_DISPLAY";

pub struct Wayland {
    splashes: Option<Splashes>,
    /// `None` keeps the daemon off Wayland, for tests.
    reach: Option<Reach>,
    retry_at: Instant,
    backoff: Duration,
}

/// How to find the compositor.
pub struct Reach {
    pub runtime_dir: PathBuf,
    pub buses: Option<Arc<Buses>>,
    pub tone: Tone,
}

impl Wayland {
    pub fn new(reach: Option<Reach>) -> Self {
        Self {
            splashes: None,
            reach,
            retry_at: Instant::now(),
            backoff: FIRST_RETRY,
        }
    }

    /// The connection, made now when it is down and a retry is due.
    pub fn get(&mut self, now: Instant) -> Option<&mut Splashes> {
        if self.splashes.is_none() && now >= self.retry_at {
            self.connect(now);
        }
        self.splashes.as_mut()
    }

    pub fn connected(&mut self) -> Option<&mut Splashes> {
        self.splashes.as_mut()
    }

    pub fn connection(&self) -> Option<&Splashes> {
        self.splashes.as_ref()
    }

    /// Drops a broken connection; the next [`Self::get`] after the backoff
    /// connects again.
    pub fn lost(&mut self, now: Instant, error: &splash::SplashError) {
        log::warn!("lost the Wayland connection: {error}");
        self.splashes = None;
        self.schedule_retry(now);
    }

    /// When the next connection attempt is due, while disconnected.
    pub fn next_retry(&self) -> Option<Instant> {
        (self.splashes.is_none() && self.reach.is_some()).then_some(self.retry_at)
    }

    pub fn is_busy(&self) -> bool {
        self.splashes.as_ref().is_some_and(Splashes::is_busy)
    }

    fn connect(&mut self, now: Instant) {
        let Some(reach) = &self.reach else {
            return;
        };
        let connected = display_socket(reach)
            .ok_or_else(|| "WAYLAND_DISPLAY is not set yet".to_owned())
            .and_then(|path| {
                UnixStream::connect(&path).map_err(|error| format!("{}: {error}", path.display()))
            })
            .and_then(|socket| {
                Splashes::connect(socket, reach.tone).map_err(|error| error.to_string())
            });
        match connected {
            Ok(splashes) => {
                log::info!("connected to the compositor");
                self.splashes = Some(splashes);
                self.backoff = FIRST_RETRY;
            }
            Err(error) => {
                log::info!(
                    "no Wayland compositor yet ({error}), retrying in {:?}",
                    self.backoff
                );
                self.schedule_retry(now);
            }
        }
    }

    fn schedule_retry(&mut self, now: Instant) {
        self.retry_at = now + self.backoff;
        self.backoff = (self.backoff * 2).min(MAX_RETRY);
    }
}

/// `WAYLAND_DISPLAY` from this process, else from the user manager, which
/// gets it when the compositor imports its environment.
fn display_socket(reach: &Reach) -> Option<PathBuf> {
    let display = std::env::var(WAYLAND_DISPLAY)
        .ok()
        .filter(|display| !display.is_empty())
        .or_else(|| {
            let session = reach.buses.as_ref()?.session().ok()?;
            let environment = Systemd(session).environment().ok()?;
            lookup(&environment, WAYLAND_DISPLAY).map(str::to_owned)
        })?;
    let path = PathBuf::from(display);
    Some(if path.is_absolute() {
        path
    } else {
        reach.runtime_dir.join(path)
    })
}
