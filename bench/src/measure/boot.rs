use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::findings::{Findings, Report};
use crate::host::Host;
use crate::host::container::ContainerCgroups;
use crate::util::clock::{millis, wait_until};
use crate::util::command;

/// How often `/proc` is scanned for the container, as in S3's `common.py`.
const SCAN_INTERVAL: Duration = Duration::from_millis(250);
/// How often `sys.boot_completed` is asked for; every ask starts Waydroid's Python.
const PROPERTY_INTERVAL: Duration = Duration::from_millis(500);
const PROPERTY_TIMEOUT: Duration = Duration::from_secs(10);
const BOOT_COMPLETED: &str = "sys.boot_completed";

/// Milestones of an Android start, from the launch call that set it off.
#[derive(Debug, Serialize)]
pub struct BootTimes {
    /// lxc-start has moved the container's init into its payload cgroup.
    pub payload_cgroup_ms: Option<f64>,
    /// `waydroid prop get sys.boot_completed` printed 1.
    pub boot_completed_ms: Option<f64>,
    pub scan_resolution_ms: f64,
    pub property_resolution_ms: f64,
    pub cgroups: ContainerCgroups,
}

impl BootTimes {
    /// Follows a start until Android reports `sys.boot_completed`, until
    /// `timeout` after `started`, or until `abandoned` is set.
    pub fn watch(host: &Host, started: Instant, timeout: Duration, abandoned: &AtomicBool) -> Self {
        let milestone = |interval: Duration, reached: &mut dyn FnMut() -> bool| {
            wait_until(started, timeout, interval, || {
                abandoned.load(Ordering::Relaxed) || reached()
            })
            .filter(|_| !abandoned.load(Ordering::Relaxed))
        };
        let payload = milestone(SCAN_INTERVAL, &mut || {
            ContainerCgroups::running(host).payload.is_some()
        });
        let completed = payload.and_then(|_| milestone(PROPERTY_INTERVAL, &mut boot_completed));
        Self {
            payload_cgroup_ms: payload.map(millis),
            boot_completed_ms: completed.map(millis),
            scan_resolution_ms: millis(SCAN_INTERVAL),
            property_resolution_ms: millis(PROPERTY_INTERVAL),
            cgroups: ContainerCgroups::running(host),
        }
    }
}

fn boot_completed() -> bool {
    command::run(
        "waydroid",
        &["prop", "get", BOOT_COMPLETED],
        PROPERTY_TIMEOUT,
    )
    .is_ok_and(|asked| asked.stdout.trim() == "1")
}

impl Report for BootTimes {
    fn report(&self, step: &str, findings: &mut Findings) {
        if self.boot_completed_ms.is_none() {
            findings.fail(step, format!("Android did not report {BOOT_COMPLETED}"));
        }
    }
}
