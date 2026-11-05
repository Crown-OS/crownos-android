//! What the bench reads from the machine, ported from S3's `measure/common.py`.

pub mod container;
#[cfg(test)]
pub mod fake;
pub mod processes;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::parse::pairs::{pairs, value_of};
use crate::parse::{pressure, proc_status};

/// The rootfs Waydroid mounts while Android runs.
const WAYDROID_ROOTFS: &str = "/var/lib/waydroid/rootfs";
/// Waydroid's network bridge.
const WAYDROID_BRIDGE: &str = "waydroid0";
/// The `/proc/meminfo` lines results keep.
const MEMINFO_KEYS: [&str; 10] = [
    "MemTotal",
    "MemFree",
    "MemAvailable",
    "Cached",
    "Shmem",
    "AnonPages",
    "SwapTotal",
    "SwapFree",
    "Zswap",
    "Zswapped",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Pid(pub u32);

impl fmt::Display for Pid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Root reads every process; a user only its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    Root,
    User,
}

impl Privilege {
    pub fn current() -> Self {
        if rustix::process::geteuid().is_root() {
            Self::Root
        } else {
            Self::User
        }
    }
}

/// Where the bench reads the machine: `/proc` and `/sys`, or a fake tree in tests.
#[derive(Debug)]
pub struct Host {
    proc: PathBuf,
    sys: PathBuf,
}

impl Host {
    pub fn system() -> Self {
        Self {
            proc: PathBuf::from("/proc"),
            sys: PathBuf::from("/sys"),
        }
    }

    #[cfg(test)]
    pub fn rooted(root: &Path) -> Self {
        Self {
            proc: root.join("proc"),
            sys: root.join("sys"),
        }
    }

    pub fn process_file(&self, pid: Pid, name: &str) -> PathBuf {
        self.proc.join(pid.to_string()).join(name)
    }

    pub fn read_process(&self, pid: Pid, name: &str) -> Option<String> {
        read(&self.process_file(pid, name))
    }

    pub fn is_alive(&self, pid: Pid) -> bool {
        self.proc.join(pid.to_string()).exists()
    }

    pub fn comm(&self, pid: Pid) -> String {
        self.read_process(pid, "comm")
            .map_or_else(|| "?".to_owned(), |comm| comm.trim_end().to_owned())
    }

    pub fn pids(&self) -> Vec<Pid> {
        numbered_entries(&self.proc)
    }

    pub fn parent_of(&self, pid: Pid) -> Option<Pid> {
        proc_status::parent_pid(&self.read_process(pid, "status")?).map(Pid)
    }

    pub fn threads(&self, pid: Pid) -> Vec<Pid> {
        numbered_entries(&self.process_file(pid, "task"))
    }

    pub fn thread_status(&self, pid: Pid, thread: Pid) -> Option<String> {
        read(
            &self
                .process_file(pid, "task")
                .join(thread.to_string())
                .join("status"),
        )
    }

    pub fn cgroup_root(&self) -> PathBuf {
        self.sys.join("fs/cgroup")
    }

    /// The cgroup v2 directory of a process.
    pub fn cgroup_of(&self, pid: Pid) -> Option<PathBuf> {
        let text = self.read_process(pid, "cgroup")?;
        let relative = text.lines().find_map(|line| line.strip_prefix("0::"))?;
        Some(
            self.cgroup_root()
                .join(relative.trim().trim_start_matches('/')),
        )
    }

    pub fn meminfo_kib(&self) -> BTreeMap<String, u64> {
        let text = read(&self.proc.join("meminfo")).unwrap_or_default();
        pairs(&text)
            .filter(|(key, _)| MEMINFO_KEYS.contains(key))
            .map(|(key, value)| (key.to_owned(), value))
            .collect()
    }

    pub fn mem_available_kib(&self) -> Option<u64> {
        value_of(&read(&self.proc.join("meminfo"))?, "MemAvailable")
    }

    pub fn memory_pressure(&self) -> BTreeMap<String, f64> {
        pressure::averages(&read(&self.proc.join("pressure/memory")).unwrap_or_default())
    }

    /// Mounts of Waydroid's rootfs and anything below it, in the bench's own namespace.
    pub fn rootfs_mounts(&self) -> usize {
        read(&self.proc.join("self/mountinfo"))
            .unwrap_or_default()
            .lines()
            .filter(|line| line.contains(WAYDROID_ROOTFS))
            .count()
    }

    pub fn bridge_state(&self) -> Option<String> {
        let path = self
            .sys
            .join("class/net")
            .join(WAYDROID_BRIDGE)
            .join("operstate");
        read(&path).map(|state| state.trim().to_owned())
    }

    pub fn kernel_release(&self) -> String {
        read(&self.proc.join("sys/kernel/osrelease"))
            .map_or_else(String::new, |release| release.trim().to_owned())
    }
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn numbered_entries(directory: &Path) -> Vec<Pid> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .map(Pid)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::fake::FakeHost;
    use super::*;

    #[test]
    fn reads_memory_mounts_and_cgroups() -> std::io::Result<()> {
        let fake = FakeHost::new("host")?;
        fake.file("proc/meminfo", include_str!("../../fixtures/meminfo"))?;
        fake.file(
            "proc/pressure/memory",
            include_str!("../../fixtures/pressure-memory"),
        )?;
        fake.file(
            "proc/self/mountinfo",
            "40 1 259:2 / / rw - ext4 /dev/nvme0n1p6 rw\n\
             912 40 7:0 / /var/lib/waydroid/rootfs ro - ext4 /dev/loop0 ro\n\
             913 912 7:1 / /var/lib/waydroid/rootfs/vendor ro - ext4 /dev/loop1 ro\n",
        )?;
        fake.process(
            4242,
            &["/system/bin/surfaceflinger"],
            1,
            "/lxc.payload.waydroid",
        )?;
        let host = fake.host();
        assert_eq!(host.meminfo_kib().len(), 10);
        assert!(host.mem_available_kib().is_some());
        assert_eq!(host.memory_pressure().len(), 6);
        assert_eq!(host.rootfs_mounts(), 2);
        assert_eq!(host.bridge_state(), None);
        assert_eq!(host.pids(), [Pid(4242)]);
        assert_eq!(host.comm(Pid(4242)), "surfaceflinger");
        assert_eq!(
            host.cgroup_of(Pid(4242)),
            Some(fake.path("sys/fs/cgroup/lxc.payload.waydroid"))
        );
        Ok(())
    }
}
