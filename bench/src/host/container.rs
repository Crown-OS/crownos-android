use std::path::{Path, PathBuf};

use serde::Serialize;

use super::processes::{Processes, Role};
use super::{Host, Pid};
use crate::parse::cpu_stat;
use crate::parse::pairs::value_of;

/// Where LXC put Waydroid's container: the payload cgroup holds Android, the
/// monitor cgroup holds lxc-start.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ContainerCgroups {
    pub payload: Option<PathBuf>,
    pub monitor: Option<PathBuf>,
}

impl ContainerCgroups {
    /// Finds them as `common.py` does: lxc-start's own cgroup, and the other
    /// cgroup its child, the container's init, moved to.
    pub fn find(host: &Host, processes: &Processes) -> Self {
        let Some(&monitor) = processes.of(Role::LxcStart).first() else {
            return Self::default();
        };
        let monitor_cgroup = host.cgroup_of(monitor);
        let payload = host
            .pids()
            .into_iter()
            .filter(|&pid| host.parent_of(pid) == Some(monitor))
            .filter_map(|pid| host.cgroup_of(pid))
            .find(|cgroup| Some(cgroup) != monitor_cgroup.as_ref());
        Self {
            payload,
            monitor: monitor_cgroup,
        }
    }

    pub fn running(host: &Host) -> Self {
        Self::find(host, &Processes::scan(host))
    }
}

/// Every process in `cgroup` and the cgroups below it.
pub fn members(cgroup: &Path) -> Vec<Pid> {
    let mut pids = Vec::new();
    collect_members(cgroup, &mut pids);
    pids
}

fn collect_members(cgroup: &Path, pids: &mut Vec<Pid>) {
    if let Ok(procs) = std::fs::read_to_string(cgroup.join("cgroup.procs")) {
        pids.extend(
            procs
                .split_whitespace()
                .filter_map(|pid| pid.parse().ok())
                .map(Pid),
        );
    }
    let Ok(entries) = std::fs::read_dir(cgroup) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_members(&entry.path(), pids);
        }
    }
}

pub fn read_number(cgroup: &Path, file: &str) -> Option<u64> {
    std::fs::read_to_string(cgroup.join(file))
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn is_frozen(cgroup: &Path) -> Option<bool> {
    let events = std::fs::read_to_string(cgroup.join("cgroup.events")).ok()?;
    value_of(&events, "frozen").map(|frozen| frozen != 0)
}

pub fn cpu_usage_usec(cgroup: &Path) -> Option<u64> {
    cpu_stat::usage_usec(&std::fs::read_to_string(cgroup.join("cpu.stat")).ok()?)
}

#[cfg(test)]
mod tests {
    use super::super::fake::FakeHost;
    use super::*;

    #[test]
    fn finds_the_payload_through_lxc_starts_child() -> std::io::Result<()> {
        let fake = FakeHost::new("container")?;
        fake.process(
            710,
            &["lxc-start", "-n", "waydroid"],
            700,
            "/lxc.monitor.waydroid",
        )?;
        fake.process(711, &["/init"], 710, "/lxc.payload.waydroid")?;
        fake.process(
            812,
            &["/system/bin/surfaceflinger"],
            711,
            "/lxc.payload.waydroid",
        )?;
        fake.process(
            1093,
            &["com.android.settings"],
            711,
            "/lxc.payload.waydroid/uid_10061/pid_1093",
        )?;
        fake.file(
            "sys/fs/cgroup/lxc.payload.waydroid/cgroup.events",
            "populated 1\nfrozen 1\n",
        )?;
        fake.file(
            "sys/fs/cgroup/lxc.payload.waydroid/cpu.stat",
            include_str!("../../fixtures/cpu.stat"),
        )?;
        fake.file(
            "sys/fs/cgroup/lxc.payload.waydroid/memory.current",
            "734003200\n",
        )?;
        let host = fake.host();
        let cgroups = ContainerCgroups::running(&host);
        let payload = fake.path("sys/fs/cgroup/lxc.payload.waydroid");
        assert_eq!(cgroups.payload.as_ref(), Some(&payload));
        assert_eq!(
            cgroups.monitor,
            Some(fake.path("sys/fs/cgroup/lxc.monitor.waydroid"))
        );
        let mut android = members(&payload);
        android.sort();
        assert_eq!(android, [Pid(711), Pid(812), Pid(1093)]);
        assert_eq!(is_frozen(&payload), Some(true));
        assert_eq!(cpu_usage_usec(&payload), Some(259_809_065));
        assert_eq!(read_number(&payload, "memory.current"), Some(734_003_200));
        assert_eq!(read_number(&payload, "memory.peak"), None);
        Ok(())
    }

    #[test]
    fn finds_nothing_while_android_is_stopped() -> std::io::Result<()> {
        let fake = FakeHost::new("stopped")?;
        fake.process(
            700,
            &["/usr/bin/waydroid", "container", "start"],
            1,
            "/system.slice/waydroid-container.service",
        )?;
        let cgroups = ContainerCgroups::running(&fake.host());
        assert!(cgroups.payload.is_none() && cgroups.monitor.is_none());
        Ok(())
    }
}
