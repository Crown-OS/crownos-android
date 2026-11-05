use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::findings::{Findings, MetricId, Report};
use crate::host::container::members;
use crate::host::processes::{Processes, Role};
use crate::host::{Host, Pid};
use crate::unit::Unit;

pub const PROCESSES: MetricId = MetricId {
    name: "idle_stop.leftover_processes",
    unit: Unit::Count,
};
pub const MOUNTS: MetricId = MetricId {
    name: "idle_stop.leftover_mounts",
    unit: Unit::Count,
};

/// What a stop left behind.
#[derive(Debug, Serialize)]
pub struct Leftovers {
    pub processes: BTreeMap<Role, Vec<Pid>>,
    /// Processes still in the payload cgroup Android ran in.
    pub payload_members: Vec<Pid>,
    pub rootfs_mounts: usize,
    pub bridge_waydroid0: Option<String>,
}

pub fn measure(host: &Host, payload: Option<&Path>) -> Leftovers {
    Leftovers {
        processes: Processes::scan(host).container(),
        payload_members: payload.map(members).unwrap_or_default(),
        rootfs_mounts: host.rootfs_mounts(),
        bridge_waydroid0: host.bridge_state(),
    }
}

impl Report for Leftovers {
    fn report(&self, _step: &str, findings: &mut Findings) {
        let processes: usize =
            self.processes.values().map(Vec::len).sum::<usize>() + self.payload_members.len();
        findings.metric(PROCESSES, processes as f64);
        findings.metric(MOUNTS, self.rootfs_mounts as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::FakeHost;

    #[test]
    fn counts_processes_and_mounts_left() -> anyhow::Result<()> {
        let fake = FakeHost::new("leftovers")?;
        fake.process(
            720,
            &["dnsmasq", "--pid-file=/run/waydroid-lxc/dnsmasq.pid"],
            1,
            "/system.slice/waydroid-container.service",
        )?;
        fake.process(
            950,
            &["/usr/bin/crown-androidd"],
            1,
            "/user.slice/crown-androidd.service",
        )?;
        fake.process(1093, &["com.android.settings"], 1, "/lxc.payload.waydroid")?;
        fake.file(
            "proc/self/mountinfo",
            "912 40 7:0 / /var/lib/waydroid/rootfs ro - ext4 /dev/loop0 ro\n",
        )?;
        fake.file("sys/class/net/waydroid0/operstate", "down\n")?;
        let host = fake.host();
        let leftovers = measure(
            &host,
            Some(&fake.path("sys/fs/cgroup/lxc.payload.waydroid")),
        );
        assert_eq!(leftovers.bridge_waydroid0.as_deref(), Some("down"));
        let mut findings = Findings::default();
        leftovers.report("leftovers", &mut findings);
        assert_eq!(
            findings.metrics[PROCESSES.name].value, 2.0,
            "dnsmasq and the app, not the daemon"
        );
        assert_eq!(findings.metrics[MOUNTS.name].value, 1.0);
        Ok(())
    }
}
