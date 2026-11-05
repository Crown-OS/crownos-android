//! The start and stop sequences, step by step.

use std::time::Duration;

use super::systemd::Systemd;
use super::{Buses, CONTAINER_SERVICE, HostError, SESSION_SERVICE, wait_for, waydroid};

/// How long the container manager gets to appear on the bus.
const CONTAINER_READY: Duration = Duration::from_secs(20);
/// How long a unit gets to become active or to stop.
const UNIT_SETTLE: Duration = Duration::from_secs(15);
/// How long the session gets to mount the images and start the container.
const SESSION_READY: Duration = Duration::from_secs(60);

/// What one session start needs. The session unit takes `WAYLAND_DISPLAY`
/// and `XDG_DATA_HOME` from the contract 10 paths itself, so the security
/// context socket and the data home must exist before it starts.
pub struct SessionPlan {
    pub venus: bool,
    /// This user.
    pub uid: u32,
}

/// Starts the container service, the render service on the Venus tier and
/// the session unit, then waits until the container runs this user's session.
pub fn start_session(buses: &Buses, plan: &SessionPlan) -> Result<(), HostError> {
    let system = buses.system()?;
    let session = buses.session()?;
    Systemd(system).start(CONTAINER_SERVICE)?;
    wait_for("the Waydroid container manager", CONTAINER_READY, || {
        Ok(waydroid::container_listening(system)?.then_some(()))
    })?;
    if let Some(uid) = waydroid::session_uid(system)?.filter(|uid| *uid != plan.uid) {
        return Err(HostError::OtherUser { uid });
    }
    if plan.venus {
        Systemd(session).start(venus::RENDER_SERVICE)?;
    }
    let systemd = Systemd(session);
    systemd.start(SESSION_SERVICE)?;
    systemd.wait_active(SESSION_SERVICE, UNIT_SETTLE)?;
    wait_for("the Waydroid session", SESSION_READY, || {
        if waydroid::session_uid(system)? == Some(plan.uid) {
            return Ok(Some(()));
        }
        match Systemd(session).active_state(SESSION_SERVICE)?.as_str() {
            "failed" | "inactive" => Err(HostError::Unit {
                unit: SESSION_SERVICE.to_owned(),
                state: "stopped before Android started".to_owned(),
            }),
            _ => Ok(None),
        }
    })
}

/// Stops everything [`start_session`] started, so no Waydroid process
/// remains. Every step runs even when an earlier one fails.
pub fn stop_session(buses: &Buses, venus: bool) {
    attempt(
        "stop the Waydroid session",
        buses.session().and_then(waydroid::stop_session),
    );
    attempt(
        "stop the session unit",
        buses
            .session()
            .and_then(|session| stop_unit(&Systemd(session), SESSION_SERVICE)),
    );
    attempt(
        "stop the container",
        buses.system().and_then(waydroid::stop_container),
    );
    attempt(
        "stop the container service",
        buses
            .system()
            .and_then(|system| stop_unit(&Systemd(system), CONTAINER_SERVICE)),
    );
    if venus {
        attempt(
            "stop the render service",
            buses
                .session()
                .and_then(|session| Systemd(session).stop(venus::RENDER_SERVICE)),
        );
    }
}

fn stop_unit(systemd: &Systemd<'_>, unit: &str) -> Result<(), HostError> {
    systemd.stop(unit)?;
    systemd.wait_inactive(unit, UNIT_SETTLE)
}

fn attempt(step: &str, result: Result<(), HostError>) {
    if let Err(error) = result {
        log::info!("could not {step}: {error}");
    }
}
