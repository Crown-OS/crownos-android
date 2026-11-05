//! Waydroid's own D-Bus services and command line.

use std::collections::HashMap;
use std::time::Duration;

use zbus::blocking::Connection;

use super::bus::call;
use super::{HostError, command};

const CONTAINER: (&str, &str, &str) = (
    "id.waydro.Container",
    "/ContainerManager",
    "id.waydro.ContainerManager",
);
const SESSION: (&str, &str, &str) = (
    "id.waydro.Session",
    "/SessionManager",
    "id.waydro.SessionManager",
);
const BUS: (&str, &str, &str) = (
    "org.freedesktop.DBus",
    "/org/freedesktop/DBus",
    "org.freedesktop.DBus",
);
const WAYDROID: &str = "waydroid";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether the container manager is on the system bus.
pub fn container_listening(system: &Connection) -> Result<bool, HostError> {
    call(system, BUS, "NameHasOwner", &(CONTAINER.0,))
}

/// The uid of the session the container runs, if any.
pub fn session_uid(system: &Connection) -> Result<Option<u32>, HostError> {
    let session: HashMap<String, String> = call(system, CONTAINER, "GetSession", &())?;
    Ok(session.get("user_id").and_then(|uid| uid.parse().ok()))
}

pub fn stop_container(system: &Connection) -> Result<(), HostError> {
    call(system, CONTAINER, "Stop", &(true,))
}

pub fn freeze(system: &Connection) -> Result<(), HostError> {
    call(system, CONTAINER, "Freeze", &())
}

pub fn unfreeze(system: &Connection) -> Result<(), HostError> {
    call(system, CONTAINER, "Unfreeze", &())
}

/// `id.waydro.SessionManager.Stop` on the session bus.
pub fn stop_session(session: &Connection) -> Result<(), HostError> {
    call(session, SESSION, "Stop", &())
}

/// Sets an Android property through the running session.
pub fn set_prop(key: &str, value: &str) -> Result<(), HostError> {
    command::run(WAYDROID, &["prop", "set", key, value], COMMAND_TIMEOUT)
}

/// Starts an Android intent through the running session.
pub fn launch_intent(action: &str, uri: &str) -> Result<(), HostError> {
    command::run(WAYDROID, &["app", "intent", action, uri], COMMAND_TIMEOUT)
}
