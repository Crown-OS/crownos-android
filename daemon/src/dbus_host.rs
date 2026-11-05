//! The real host: systemd, Waydroid and logind over D-Bus.

use std::os::fd::OwnedFd;
use std::sync::Arc;

use runtime::host::systemd::Systemd;
use runtime::host::{self, Buses, HostError, SETUP_SERVICE, SessionPlan, logind, waydroid};
use runtime::lifecycle::BootFailure;

use crate::host_worker::Host;
use crate::notice::{Notice, Notifier};

const SAFE_MODE_PROP: &str = "persist.crownos.safe_mode";

pub struct DbusHost {
    buses: Arc<Buses>,
    uid: u32,
    inhibitor: Option<OwnedFd>,
    following_sleep: bool,
}

impl DbusHost {
    pub fn new(buses: Arc<Buses>) -> Self {
        Self {
            buses,
            uid: rustix::process::getuid().as_raw(),
            inhibitor: None,
            following_sleep: false,
        }
    }

    fn follow_sleep(&mut self, notifier: &Notifier) -> Result<(), HostError> {
        if self.following_sleep {
            return Ok(());
        }
        let notifier = notifier.clone();
        logind::follow_sleep(self.buses.system()?, move |sleeping| {
            notifier.notify(Notice::Sleep(sleeping));
        })?;
        self.following_sleep = true;
        Ok(())
    }
}

impl Host for DbusHost {
    fn start_session(&mut self, venus: bool) -> Result<(), BootFailure> {
        let plan = SessionPlan {
            venus,
            uid: self.uid,
        };
        host::start_session(&self.buses, &plan).map_err(|error| match error {
            HostError::OtherUser { uid } => BootFailure::OtherUser { uid },
            other => BootFailure::Host(other.to_string()),
        })
    }

    fn stop_session(&mut self, venus: bool) {
        host::stop_session(&self.buses, venus);
    }

    fn set_safe_mode(&mut self, enabled: bool) {
        let value = if enabled { "1" } else { "0" };
        logged("set safe mode", waydroid::set_prop(SAFE_MODE_PROP, value));
    }

    fn freeze(&mut self) {
        logged(
            "freeze Android",
            self.buses.system().and_then(waydroid::freeze),
        );
    }

    fn thaw(&mut self, venus: bool) {
        logged(
            "unfreeze Android",
            self.buses.system().and_then(waydroid::unfreeze),
        );
        if venus {
            let restarted = self
                .buses
                .session()
                .and_then(|session| Systemd(session).restart(venus::RENDER_SERVICE));
            logged("restart the render service", restarted);
        }
    }

    fn hold_sleep_inhibitor(&mut self, notifier: &Notifier) {
        logged("follow sleep", self.follow_sleep(notifier));
        if self.inhibitor.is_none() {
            match self.buses.system().and_then(logind::inhibit_sleep) {
                Ok(fd) => self.inhibitor = Some(fd),
                Err(error) => log::warn!("could not delay sleep: {error}"),
            }
        }
    }

    fn release_sleep_inhibitor(&mut self) {
        self.inhibitor = None;
    }

    fn launch_intent(&mut self, action: &str, uri: &str) {
        logged("start the intent", waydroid::launch_intent(action, uri));
    }

    fn retry_setup(&mut self) {
        let started = self
            .buses
            .system()
            .and_then(|system| Systemd(system).start(SETUP_SERVICE));
        logged("start the setup service", started);
    }
}

fn logged(step: &str, result: Result<(), HostError>) {
    if let Err(error) = result {
        log::warn!("could not {step}: {error}");
    }
}
