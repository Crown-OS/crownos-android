use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;

use super::launch::ColdStart;
use super::{CLI, NOT_RUNNING};
use crate::findings::{Findings, MetricId, Report};
use crate::host::Host;
use crate::host::container::{self, ContainerCgroups};
use crate::host::processes::{Processes, Role};
use crate::unit::Unit;
use crate::util::clock::{millis, wait_until};
use crate::util::command;

pub const STOP: MetricId = MetricId {
    name: "idle_stop.stop",
    unit: Unit::Milliseconds,
};

/// Waydroid's container manager on the system bus, whose policy lets every local user call it.
const BUSCTL_CONTAINER: [&str; 5] = [
    "--system",
    "call",
    "id.waydro.Container",
    "/ContainerManager",
    "id.waydro.ContainerManager",
];
const FREEZE_TIMEOUT: Duration = Duration::from_secs(30);
const FREEZE_POLL: Duration = Duration::from_millis(5);
const STOP_POLL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug)]
enum ContainerCall {
    Freeze,
    Unfreeze,
}

impl ContainerCall {
    const fn method(self) -> &'static str {
        match self {
            Self::Freeze => "Freeze",
            Self::Unfreeze => "Unfreeze",
        }
    }

    const fn leaves_frozen(self) -> bool {
        matches!(self, Self::Freeze)
    }
}

/// One Freeze or Unfreeze call.
#[derive(Debug, Serialize)]
pub struct Toggle {
    pub call_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// From the call's return until `cgroup.events` shows the new state.
    pub cgroup_settled_after_call_ms: Option<f64>,
}

impl Toggle {
    fn problem(&self) -> Option<&str> {
        match (&self.error, self.cgroup_settled_after_call_ms) {
            (Some(error), _) => Some(error),
            (None, None) => Some("the payload cgroup never changed state"),
            (None, Some(_)) => None,
        }
    }
}

/// Freeze, hold, unfreeze, as S3's `lifecycle.py` does for suspend.
#[derive(Debug, Serialize)]
pub struct FreezeCycle {
    pub freeze: Toggle,
    pub cpu_usec_while_frozen: Option<u64>,
    pub mem_available_frozen_kib: Option<u64>,
    pub unfreeze: Toggle,
}

pub fn freeze_cycle(
    host: &Host,
    cgroups: &ContainerCgroups,
    hold: Duration,
) -> Result<FreezeCycle> {
    let payload = cgroups.payload.as_deref().context(NOT_RUNNING)?;
    let freeze = toggle(payload, ContainerCall::Freeze)?;
    let before = container::cpu_usage_usec(payload);
    thread::sleep(hold);
    let after = container::cpu_usage_usec(payload);
    let mem_available_frozen_kib = host.mem_available_kib();
    let unfreeze = toggle(payload, ContainerCall::Unfreeze)?;
    Ok(FreezeCycle {
        freeze,
        cpu_usec_while_frozen: after
            .zip(before)
            .map(|(after, before)| after.saturating_sub(before)),
        mem_available_frozen_kib,
        unfreeze,
    })
}

fn toggle(payload: &Path, call: ContainerCall) -> Result<Toggle> {
    let args: Vec<&str> = BUSCTL_CONTAINER
        .iter()
        .copied()
        .chain([call.method()])
        .collect();
    let done = command::run("busctl", &args, FREEZE_TIMEOUT)?;
    let settled = wait_until(Instant::now(), FREEZE_TIMEOUT, FREEZE_POLL, || {
        container::is_frozen(payload) == Some(call.leaves_frozen())
    });
    Ok(Toggle {
        call_ms: millis(done.elapsed),
        error: (!done.success()).then(|| done.error()),
        cgroup_settled_after_call_ms: settled.map(millis),
    })
}

impl Report for FreezeCycle {
    fn report(&self, step: &str, findings: &mut Findings) {
        for (call, toggle) in [("freeze", &self.freeze), ("unfreeze", &self.unfreeze)] {
            if let Some(problem) = toggle.problem() {
                findings.fail(&format!("{step} {call}"), problem);
            }
        }
    }
}

/// One `crown-android stop`, timed from the call until nothing of the container is left.
#[derive(Debug, Serialize)]
pub struct StopTimes {
    pub call_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub payload_cgroup: Option<PathBuf>,
    /// The payload cgroup and lxc-start are gone.
    pub container_gone_ms: Option<f64>,
    pub rootfs_unmounted_ms: Option<f64>,
    /// No Waydroid process is left.
    pub processes_gone_ms: Option<f64>,
}

impl StopTimes {
    /// When the last trace of the container went, if all went before the timeout.
    pub fn everything_gone_ms(&self) -> Option<f64> {
        [
            self.container_gone_ms,
            self.rootfs_unmounted_ms,
            self.processes_gone_ms,
        ]
        .into_iter()
        .try_fold(0.0, |latest, gone| gone.map(|gone| f64::max(latest, gone)))
    }

    fn report_problems(&self, step: &str, findings: &mut Findings) {
        if let Some(error) = &self.error {
            findings.fail(step, error.as_str());
        } else if self.everything_gone_ms().is_none() {
            findings.fail(
                step,
                "Waydroid processes or mounts were left after the stop",
            );
        }
    }
}

impl Report for StopTimes {
    fn report(&self, step: &str, findings: &mut Findings) {
        self.report_problems(step, findings);
    }
}

/// The stop the idle-stop budget judges.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct BudgetedStop(pub StopTimes);

impl Report for BudgetedStop {
    fn report(&self, step: &str, findings: &mut Findings) {
        self.0.report_problems(step, findings);
        if let Some(gone) = self.0.everything_gone_ms() {
            findings.metric(STOP, gone);
        }
    }
}

/// Stops Android through the daemon, the same sequence its idle timer runs.
pub fn stop(host: &Host, timeout: Duration) -> Result<StopTimes> {
    let processes = Processes::scan(host);
    let monitors = processes.of(Role::LxcStart).to_vec();
    let payload = ContainerCgroups::find(host, &processes).payload;
    let started = Instant::now();
    let done = command::run(CLI, &["stop"], timeout)?;
    let gone = |left: &mut dyn FnMut() -> bool| {
        wait_until(started, timeout, STOP_POLL, || !left()).map(millis)
    };
    let container_gone_ms = gone(&mut || {
        payload.as_deref().is_some_and(Path::exists)
            || monitors.iter().any(|&pid| host.is_alive(pid))
    });
    let rootfs_unmounted_ms = gone(&mut || host.rootfs_mounts() > 0);
    let processes_gone_ms = gone(&mut || !Processes::scan(host).container().is_empty());
    Ok(StopTimes {
        call_ms: millis(done.elapsed),
        error: (!done.success()).then(|| done.error()),
        payload_cgroup: payload,
        container_gone_ms,
        rootfs_unmounted_ms,
        processes_gone_ms,
    })
}

/// Whether anything of the container runs.
pub fn is_running(host: &Host) -> bool {
    !Processes::scan(host).container().is_empty()
}

/// Android started again after a stop, with warm caches.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Restart(pub ColdStart);

impl Report for Restart {
    fn report(&self, step: &str, findings: &mut Findings) {
        self.0.boot.report(step, findings);
        if let Some(error) = &self.0.launch.error {
            findings.fail(step, error.as_str());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stopped(container: Option<f64>, rootfs: Option<f64>, processes: Option<f64>) -> StopTimes {
        StopTimes {
            call_ms: 120.0,
            error: None,
            payload_cgroup: None,
            container_gone_ms: container,
            rootfs_unmounted_ms: rootfs,
            processes_gone_ms: processes,
        }
    }

    #[test]
    fn judges_a_stop_by_its_last_trace() {
        let clean = stopped(Some(2_100.0), Some(2_300.0), Some(2_250.0));
        assert_eq!(clean.everything_gone_ms(), Some(2_300.0));
        let mut findings = Findings::default();
        BudgetedStop(clean).report("stop", &mut findings);
        assert_eq!(findings.metrics[STOP.name].value, 2_300.0);
        assert!(findings.failures.is_empty());

        let mut findings = Findings::default();
        BudgetedStop(stopped(Some(2_100.0), None, Some(2_250.0))).report("stop", &mut findings);
        assert!(findings.metrics.is_empty());
        assert!(findings.failures.contains_key("stop"));
    }

    #[test]
    fn reports_failed_toggles() {
        let cycle = FreezeCycle {
            freeze: Toggle {
                call_ms: 40.0,
                error: None,
                cgroup_settled_after_call_ms: Some(3.0),
            },
            cpu_usec_while_frozen: Some(0),
            mem_available_frozen_kib: None,
            unfreeze: Toggle {
                call_ms: 30.0,
                error: Some("Call failed: Access denied".to_owned()),
                cgroup_settled_after_call_ms: None,
            },
        };
        let mut findings = Findings::default();
        cycle.report("freeze", &mut findings);
        assert_eq!(findings.failures.len(), 1);
        assert_eq!(
            findings.failures["freeze unfreeze"],
            "Call failed: Access denied"
        );
    }
}
