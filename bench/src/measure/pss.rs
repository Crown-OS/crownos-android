use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;

use serde::Serialize;

use super::{NEEDS_ROOT, NOT_RUNNING};
use crate::findings::{Findings, MetricId, Report};
use crate::host::container::{ContainerCgroups, members};
use crate::host::processes::{Processes, Role, arguments, basename};
use crate::host::{Host, Pid, Privilege};
use crate::parse::smaps_rollup::SmapsRollup;
use crate::unit::Unit;
use crate::util::number::mib_from_kib;

pub const HWC: MetricId = MetricId {
    name: "memory.hwc",
    unit: Unit::Mebibytes,
};
pub const AGENT: MetricId = MetricId {
    name: "memory.agent",
    unit: Unit::Mebibytes,
};
pub const DAEMON: MetricId = MetricId {
    name: "memory.daemon",
    unit: Unit::Mebibytes,
};

/// How many of Android's largest processes results list.
const TOP_PROCESSES: usize = 15;
/// The composer HAL service that loads `hwcomposer.waydroid.so`.
const COMPOSER_PREFIX: &str = "android.hardware.graphics.composer";
/// crown-agent runs through `app_process` with a class of this package, or under this nice name.
const AGENT_PACKAGE: &str = "org.crownos.agent";
const AGENT_NAME: &str = "crown-agent";

/// An Android process with a budget of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Component {
    Hwc,
    Agent,
}

impl Component {
    fn of(argv: &[&str]) -> Option<Self> {
        let program = argv.first().map_or("", |program| basename(program));
        if program.starts_with(COMPOSER_PREFIX) {
            return Some(Self::Hwc);
        }
        let agent = program == AGENT_NAME
            || argv
                .iter()
                .any(|argument| argument.starts_with(AGENT_PACKAGE));
        agent.then_some(Self::Agent)
    }
}

/// One process's `smaps_rollup`, or why it could not be read.
#[derive(Clone, Debug, Serialize)]
pub struct ProcessPss {
    pub pid: Pid,
    pub comm: String,
    #[serde(flatten)]
    pub rollup: Option<SmapsRollup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ProcessPss {
    fn read(host: &Host, pid: Pid) -> Self {
        let (rollup, error) = match std::fs::read_to_string(host.process_file(pid, "smaps_rollup"))
        {
            Ok(text) => match SmapsRollup::parse(&text) {
                Some(rollup) => (Some(rollup), None),
                None => (None, Some("no Pss in smaps_rollup".to_owned())),
            },
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                (None, Some(NEEDS_ROOT.to_owned()))
            }
            Err(error) => (None, Some(error.to_string())),
        };
        Self {
            pid,
            comm: host.comm(pid),
            rollup,
            error,
        }
    }
}

/// Sums a field over the readable rollups; `None` when there are none.
fn total(processes: &[ProcessPss], field: fn(&SmapsRollup) -> u64) -> Option<u64> {
    processes
        .iter()
        .filter_map(|process| process.rollup.as_ref().map(field))
        .reduce(|sum, kib| sum + kib)
}

/// Android's own processes, as `memory.py` sums them as root.
#[derive(Debug, Serialize)]
pub struct AndroidPss {
    pub processes: usize,
    pub unreadable: usize,
    pub pss_kib: u64,
    pub swap_pss_kib: u64,
    pub top: Vec<ProcessPss>,
    pub hwc: Vec<ProcessPss>,
    pub agent: Vec<ProcessPss>,
}

impl AndroidPss {
    fn read(host: &Host, payload: &Path) -> Self {
        let mut hwc = Vec::new();
        let mut agent = Vec::new();
        let mut readable = Vec::new();
        let mut unreadable = 0;
        for pid in members(payload) {
            let process = ProcessPss::read(host, pid);
            let command_line = host.read_process(pid, "cmdline").unwrap_or_default();
            match Component::of(&arguments(&command_line)) {
                Some(Component::Hwc) => hwc.push(process.clone()),
                Some(Component::Agent) => agent.push(process.clone()),
                None => {}
            }
            match process.rollup {
                Some(_) => readable.push(process),
                None => unreadable += 1,
            }
        }
        readable.sort_by_key(|process| {
            std::cmp::Reverse(process.rollup.map_or(0, |rollup| rollup.pss_kib))
        });
        Self {
            processes: readable.len() + unreadable,
            unreadable,
            pss_kib: total(&readable, |rollup| rollup.pss_kib).unwrap_or(0),
            swap_pss_kib: total(&readable, |rollup| rollup.swap_pss_kib).unwrap_or(0),
            top: readable.into_iter().take(TOP_PROCESSES).collect(),
            hwc,
            agent,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidReading {
    Measured(AndroidPss),
    Skipped(&'static str),
}

/// PSS of the runtime's host processes and, as root, of Android's.
#[derive(Debug, Serialize)]
pub struct Pss {
    pub host: BTreeMap<Role, Vec<ProcessPss>>,
    pub android: AndroidReading,
}

pub fn measure(
    host: &Host,
    privilege: Privilege,
    processes: &Processes,
    cgroups: &ContainerCgroups,
) -> Pss {
    let mut by_role: BTreeMap<Role, Vec<ProcessPss>> = BTreeMap::new();
    for (role, pid) in processes.iter() {
        by_role
            .entry(role)
            .or_default()
            .push(ProcessPss::read(host, pid));
    }
    let android = match (privilege, cgroups.payload.as_deref()) {
        (Privilege::User, _) => AndroidReading::Skipped(NEEDS_ROOT),
        (Privilege::Root, None) => AndroidReading::Skipped(NOT_RUNNING),
        (Privilege::Root, Some(payload)) => {
            AndroidReading::Measured(AndroidPss::read(host, payload))
        }
    };
    Pss {
        host: by_role,
        android,
    }
}

fn record(
    findings: &mut Findings,
    metric: MetricId,
    processes: &[ProcessPss],
    field: fn(&SmapsRollup) -> u64,
    missing: &str,
) {
    match total(processes, field) {
        Some(kib) => findings.metric(metric, mib_from_kib(kib as f64)),
        None => findings.skip(metric, missing),
    }
}

impl Report for Pss {
    fn report(&self, _step: &str, findings: &mut Findings) {
        let daemon = self.host.get(&Role::Daemon).map_or(&[][..], Vec::as_slice);
        record(
            findings,
            DAEMON,
            daemon,
            |rollup| rollup.pss_kib,
            "no readable crown-androidd process",
        );
        match &self.android {
            AndroidReading::Skipped(reason) => {
                findings.skip(HWC, *reason);
                findings.skip(AGENT, *reason);
            }
            AndroidReading::Measured(android) => {
                record(
                    findings,
                    HWC,
                    &android.hwc,
                    |rollup| rollup.rss_kib,
                    "no composer HAL process was found",
                );
                record(
                    findings,
                    AGENT,
                    &android.agent,
                    |rollup| rollup.pss_kib,
                    "no crown-agent process was found",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::FakeHost;

    const ROLLUP: &str = include_str!("../../fixtures/smaps_rollup");

    #[test]
    fn tells_android_components_apart() {
        assert_eq!(
            Component::of(&["/vendor/bin/hw/android.hardware.graphics.composer@2.1-service"]),
            Some(Component::Hwc)
        );
        assert_eq!(
            Component::of(&["/system/bin/app_process64", "/", "org.crownos.agent.Main"]),
            Some(Component::Agent)
        );
        assert_eq!(Component::of(&["crown-agent"]), Some(Component::Agent));
        assert_eq!(Component::of(&["/system/bin/surfaceflinger"]), None);
    }

    #[test]
    fn sums_android_and_names_components_as_root() -> anyhow::Result<()> {
        let fake = FakeHost::new("pss")?;
        fake.process(
            710,
            &["lxc-start", "-n", "waydroid"],
            1,
            "/lxc.monitor.waydroid",
        )?;
        fake.process(711, &["/init"], 710, "/lxc.payload.waydroid")?;
        fake.process(
            812,
            &["/vendor/bin/hw/android.hardware.graphics.composer@2.1-service"],
            711,
            "/lxc.payload.waydroid",
        )?;
        fake.process(
            900,
            &["/system/bin/app_process64", "/", "org.crownos.agent.Main"],
            711,
            "/lxc.payload.waydroid",
        )?;
        fake.process(
            950,
            &["/usr/bin/crown-androidd"],
            1,
            "/user.slice/crown-androidd.service",
        )?;
        for pid in [711, 812, 900, 950] {
            fake.file(&format!("proc/{pid}/smaps_rollup"), ROLLUP)?;
        }
        let host = fake.host();
        let processes = Processes::scan(&host);
        let cgroups = ContainerCgroups::find(&host, &processes);
        let pss = measure(&host, Privilege::Root, &processes, &cgroups);
        let AndroidReading::Measured(android) = &pss.android else {
            panic!("Android was not measured");
        };
        assert_eq!(
            (android.processes, android.unreadable, android.pss_kib),
            (3, 0, 765)
        );
        let mut findings = Findings::default();
        pss.report("pss", &mut findings);
        let value = |metric: MetricId| findings.metrics.get(metric.name).map(|m| m.value);
        assert_eq!(value(HWC), Some(4.04));
        assert_eq!(value(AGENT), Some(0.25));
        assert_eq!(value(DAEMON), Some(0.25));
        assert!(
            pss.host
                .get(&Role::LxcStart)
                .is_some_and(|lxc| lxc[0].error.is_some())
        );

        let user = measure(&host, Privilege::User, &processes, &cgroups);
        let mut findings = Findings::default();
        user.report("pss", &mut findings);
        assert_eq!(findings.skipped[HWC.name], NEEDS_ROOT);
        assert!(findings.metrics.contains_key(DAEMON.name));
        Ok(())
    }
}
