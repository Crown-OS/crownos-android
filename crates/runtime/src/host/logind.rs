//! Sleep: a delay inhibitor and the `PrepareForSleep` signal of
//! `org.freedesktop.login1`.

use std::os::fd::OwnedFd;
use std::thread::JoinHandle;

use zbus::blocking::{Connection, MessageIterator};

use super::HostError;
use super::bus::call;

const MANAGER: (&str, &str, &str) = (
    "org.freedesktop.login1",
    "/org/freedesktop/login1",
    "org.freedesktop.login1.Manager",
);
const PREPARE_FOR_SLEEP: &str = "type='signal',sender='org.freedesktop.login1',\
    path='/org/freedesktop/login1',interface='org.freedesktop.login1.Manager',\
    member='PrepareForSleep'";

/// Delays sleep until the returned descriptor is closed.
pub fn inhibit_sleep(system: &Connection) -> Result<OwnedFd, HostError> {
    let fd: zbus::zvariant::OwnedFd = call(
        system,
        MANAGER,
        "Inhibit",
        &(
            "sleep",
            "crown-androidd",
            "Freeze Android before sleep",
            "delay",
        ),
    )?;
    Ok(fd.into())
}

/// Calls `on_change(true)` before sleep and `on_change(false)` after resume,
/// from a thread of its own.
pub fn follow_sleep(
    system: &Connection,
    on_change: impl Fn(bool) + Send + 'static,
) -> Result<JoinHandle<()>, HostError> {
    let signals = MessageIterator::for_match_rule(PREPARE_FOR_SLEEP, system, None)?;
    Ok(std::thread::Builder::new()
        .name("sleep".into())
        .spawn(move || {
            for message in signals.flatten() {
                match message.body().deserialize::<bool>() {
                    Ok(sleeping) => on_change(sleeping),
                    Err(error) => log::warn!("bad PrepareForSleep signal: {error}"),
                }
            }
        })?)
}
