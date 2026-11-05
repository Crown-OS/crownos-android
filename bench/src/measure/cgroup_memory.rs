use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use super::NOT_RUNNING;
use crate::findings::{Findings, MetricId, Report};
use crate::host::Host;
use crate::host::container::{ContainerCgroups, read_number};
use crate::parse::pairs::all_pairs;
use crate::unit::Unit;
use crate::util::number::mib_from_bytes;

pub const ANDROID: MetricId = MetricId {
    name: "memory.android",
    unit: Unit::Mebibytes,
};

/// A cgroup's memory files, in bytes, as S3's `memory.py` reads them.
#[derive(Debug, Serialize)]
pub struct CgroupMemory {
    pub path: PathBuf,
    pub current: Option<u64>,
    pub peak: Option<u64>,
    pub swap_current: Option<u64>,
    pub zswap_current: Option<u64>,
    pub stat: BTreeMap<String, u64>,
}

impl CgroupMemory {
    pub fn read(cgroup: &Path) -> Self {
        let stat = std::fs::read_to_string(cgroup.join("memory.stat")).unwrap_or_default();
        Self {
            path: cgroup.to_owned(),
            current: read_number(cgroup, "memory.current"),
            peak: read_number(cgroup, "memory.peak"),
            swap_current: read_number(cgroup, "memory.swap.current"),
            zswap_current: read_number(cgroup, "memory.zswap.current"),
            stat: all_pairs(&stat),
        }
    }
}

/// The container's memory while its apps run.
#[derive(Debug, Serialize)]
pub struct ContainerMemory {
    pub android_apps: Option<u32>,
    pub payload: CgroupMemory,
    pub monitor: Option<CgroupMemory>,
    pub meminfo_kib: BTreeMap<String, u64>,
    pub memory_pressure: BTreeMap<String, f64>,
}

pub fn measure(
    host: &Host,
    cgroups: &ContainerCgroups,
    android_apps: Option<u32>,
) -> Result<ContainerMemory> {
    let payload = cgroups.payload.as_deref().context(NOT_RUNNING)?;
    Ok(ContainerMemory {
        android_apps,
        payload: CgroupMemory::read(payload),
        monitor: cgroups.monitor.as_deref().map(CgroupMemory::read),
        meminfo_kib: host.meminfo_kib(),
        memory_pressure: host.memory_pressure(),
    })
}

impl Report for ContainerMemory {
    fn report(&self, step: &str, findings: &mut Findings) {
        match self.payload.current {
            Some(bytes) => {
                findings.metric_with_apps(ANDROID, mib_from_bytes(bytes), self.android_apps)
            }
            None => findings.fail(step, "memory.current of the payload cgroup is unreadable"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::FakeHost;

    #[test]
    fn reports_the_payload_cgroup() -> anyhow::Result<()> {
        let fake = FakeHost::new("cgroup-memory")?;
        fake.process(
            710,
            &["lxc-start", "-n", "waydroid"],
            1,
            "/lxc.monitor.waydroid",
        )?;
        fake.process(711, &["/init"], 710, "/lxc.payload.waydroid")?;
        fake.file(
            "sys/fs/cgroup/lxc.payload.waydroid/memory.current",
            "1258291200\n",
        )?;
        fake.file(
            "sys/fs/cgroup/lxc.payload.waydroid/memory.stat",
            include_str!("../../fixtures/memory.stat"),
        )?;
        let host = fake.host();
        let memory = measure(&host, &ContainerCgroups::running(&host), Some(2))?;
        assert_eq!(memory.payload.stat.get("anon"), Some(&317_427_712));
        assert!(
            memory
                .monitor
                .is_some_and(|monitor| monitor.current.is_none())
        );
        let mut findings = Findings::default();
        let memory = measure(&host, &ContainerCgroups::running(&host), Some(2))?;
        memory.report("memory", &mut findings);
        let android = findings.metrics.get(ANDROID.name);
        assert_eq!(
            android.map(|m| (m.value, m.android_apps)),
            Some((1200.0, Some(2)))
        );
        assert!(measure(&host, &ContainerCgroups::default(), None).is_err());
        Ok(())
    }
}
