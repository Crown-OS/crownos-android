//! `org.freedesktop.systemd1` on the system or the session bus.

use std::time::Duration;

use zbus::blocking::Connection;
use zbus::zvariant::OwnedObjectPath;

use super::bus::{call, property};
use super::{HostError, wait_for};

const DESTINATION: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const UNIT: &str = "org.freedesktop.systemd1.Unit";
const JOB_MODE: &str = "replace";

/// The systemd instance behind one bus connection.
pub struct Systemd<'a>(pub &'a Connection);

impl Systemd<'_> {
    pub fn start(&self, unit: &str) -> Result<(), HostError> {
        self.job("StartUnit", unit)
    }

    pub fn stop(&self, unit: &str) -> Result<(), HostError> {
        self.job("StopUnit", unit)
    }

    pub fn restart(&self, unit: &str) -> Result<(), HostError> {
        self.job("RestartUnit", unit)
    }

    /// `ActiveState` of `unit`: `active`, `inactive`, `failed`, `activating`...
    pub fn active_state(&self, unit: &str) -> Result<String, HostError> {
        let path: OwnedObjectPath = call(self.0, manager(), "LoadUnit", &(unit,))?;
        property(self.0, (DESTINATION, path.as_str(), UNIT), "ActiveState")
    }

    /// Waits for `unit` to be active, failing early when it fails.
    pub fn wait_active(&self, unit: &str, timeout: Duration) -> Result<(), HostError> {
        wait_for(&format!("{unit} becoming active"), timeout, || {
            match self.active_state(unit)?.as_str() {
                "active" => Ok(Some(())),
                "failed" => Err(HostError::Unit {
                    unit: unit.to_owned(),
                    state: "failed".to_owned(),
                }),
                _ => Ok(None),
            }
        })
    }

    pub fn wait_inactive(&self, unit: &str, timeout: Duration) -> Result<(), HostError> {
        wait_for(&format!("{unit} stopping"), timeout, || {
            Ok(matches!(self.active_state(unit)?.as_str(), "inactive" | "failed").then_some(()))
        })
    }

    /// The manager's environment block, `KEY=value` each.
    pub fn environment(&self) -> Result<Vec<String>, HostError> {
        property(self.0, manager(), "Environment")
    }

    fn job(&self, method: &str, unit: &str) -> Result<(), HostError> {
        let _job: OwnedObjectPath = call(self.0, manager(), method, &(unit, JOB_MODE))?;
        Ok(())
    }
}

const fn manager() -> (&'static str, &'static str, &'static str) {
    (DESTINATION, MANAGER_PATH, MANAGER)
}

/// The value `environment` gives `key`, if any.
pub fn lookup<'a>(environment: &'a [String], key: &str) -> Option<&'a str> {
    environment.iter().find_map(|assignment| {
        assignment
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
    })
}

#[cfg(test)]
mod tests {
    use super::lookup;

    #[test]
    fn finds_a_variable_without_matching_its_prefixes() {
        let environment = [
            "WAYLAND_DISPLAY_X=1".to_owned(),
            "WAYLAND_DISPLAY=wayland-1".to_owned(),
        ];
        assert_eq!(lookup(&environment, "WAYLAND_DISPLAY"), Some("wayland-1"));
        assert_eq!(lookup(&environment, "XDG_DATA_HOME"), None);
    }
}
