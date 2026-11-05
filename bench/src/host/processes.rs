use std::collections::BTreeMap;

use serde::Serialize;

use super::{Host, Pid};

/// What a host process does for the Android runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// `waydroid container start`, run by `waydroid-container.service`.
    ContainerManager,
    /// `waydroid session start`, run by `crownos-android-session.service`.
    SessionManager,
    /// The LXC monitor of Waydroid's container.
    LxcStart,
    /// The DHCP server on Waydroid's bridge.
    Dnsmasq,
    Sensord,
    /// `crown-androidd`, which outlives Android by design.
    Daemon,
}

const DAEMON_PROGRAM: &str = "crown-androidd";
const WAYDROID_LXC_RUNTIME: &str = "/run/waydroid-lxc/";

impl Role {
    /// The name results use, as serialized.
    pub const fn name(self) -> &'static str {
        match self {
            Self::ContainerManager => "container_manager",
            Self::SessionManager => "session_manager",
            Self::LxcStart => "lxc_start",
            Self::Dnsmasq => "dnsmasq",
            Self::Sensord => "sensord",
            Self::Daemon => "daemon",
        }
    }

    pub const fn is_container(self) -> bool {
        !matches!(self, Self::Daemon)
    }

    /// The role behind a command line, as `common.py`'s `role_of` tells them apart.
    pub fn of(argv: &[&str]) -> Option<Self> {
        let program = argv.first().map_or("", |program| basename(program));
        let waydroid_starts = |command: &str| {
            argv.windows(3).any(|words| {
                basename(words[0]) == "waydroid" && words[1] == command && words[2] == "start"
            })
        };
        if waydroid_starts("container") {
            return Some(Self::ContainerManager);
        }
        if waydroid_starts("session") {
            return Some(Self::SessionManager);
        }
        match program {
            "lxc-start" if argv.contains(&"waydroid") => Some(Self::LxcStart),
            "dnsmasq"
                if argv
                    .iter()
                    .any(|argument| argument.contains(WAYDROID_LXC_RUNTIME)) =>
            {
                Some(Self::Dnsmasq)
            }
            "waydroid-sensord" => Some(Self::Sensord),
            DAEMON_PROGRAM => Some(Self::Daemon),
            _ => None,
        }
    }
}

pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The arguments of a `/proc/<pid>/cmdline`.
pub fn arguments(command_line: &str) -> Vec<&str> {
    command_line
        .split('\0')
        .filter(|argument| !argument.is_empty())
        .collect()
}

/// The host processes of the Android runtime, by role.
#[derive(Debug, Default, Serialize)]
#[serde(transparent)]
pub struct Processes(BTreeMap<Role, Vec<Pid>>);

impl Processes {
    pub fn scan(host: &Host) -> Self {
        let mut processes = Self::default();
        for pid in host.pids() {
            let Some(command_line) = host.read_process(pid, "cmdline") else {
                continue;
            };
            if let Some(role) = Role::of(&arguments(&command_line)) {
                processes.0.entry(role).or_default().push(pid);
            }
        }
        processes
    }

    pub fn of(&self, role: Role) -> &[Pid] {
        self.0.get(&role).map_or(&[], Vec::as_slice)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Role, Pid)> + '_ {
        self.0
            .iter()
            .flat_map(|(role, pids)| pids.iter().map(move |pid| (*role, *pid)))
    }

    /// The container's processes: everything but the daemon.
    pub fn container(&self) -> BTreeMap<Role, Vec<Pid>> {
        self.0
            .iter()
            .filter(|(role, _)| role.is_container())
            .map(|(role, pids)| (*role, pids.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::fake::FakeHost;
    use super::*;

    #[test]
    fn tells_roles_apart() {
        let role = |argv: &[&str]| Role::of(argv);
        assert_eq!(
            role(&[
                "/usr/bin/python3",
                "/usr/bin/waydroid",
                "container",
                "start"
            ]),
            Some(Role::ContainerManager)
        );
        assert_eq!(
            role(&["python3", "/usr/bin/waydroid", "session", "start"]),
            Some(Role::SessionManager)
        );
        assert_eq!(
            role(&["/usr/bin/waydroid", "session", "stop"]),
            None,
            "a stop call is not the session"
        );
        assert_eq!(
            role(&[
                "lxc-start",
                "-P",
                "/var/lib/waydroid/lxc",
                "-F",
                "-n",
                "waydroid",
                "--"
            ]),
            Some(Role::LxcStart)
        );
        assert_eq!(role(&["lxc-start", "-n", "debian"]), None);
        assert_eq!(
            role(&["dnsmasq", "--pid-file=/run/waydroid-lxc/dnsmasq.pid"]),
            Some(Role::Dnsmasq)
        );
        assert_eq!(
            role(&["/usr/bin/dnsmasq", "--conf-file=/etc/dnsmasq.conf"]),
            None
        );
        assert_eq!(role(&["/usr/bin/crown-androidd"]), Some(Role::Daemon));
        assert_eq!(role(&[]), None);
        assert!(!Role::Daemon.is_container());
        for role in [
            Role::ContainerManager,
            Role::SessionManager,
            Role::LxcStart,
            Role::Dnsmasq,
            Role::Sensord,
            Role::Daemon,
        ] {
            assert_eq!(
                serde_json::to_value(role).ok(),
                Some(serde_json::json!(role.name()))
            );
        }
    }

    #[test]
    fn scans_the_runtime() -> std::io::Result<()> {
        let fake = FakeHost::new("scan")?;
        fake.process(
            700,
            &[
                "/usr/bin/python3",
                "/usr/bin/waydroid",
                "container",
                "start",
            ],
            1,
            "/system.slice/waydroid-container.service",
        )?;
        fake.process(
            710,
            &["lxc-start", "-n", "waydroid"],
            700,
            "/lxc.monitor.waydroid",
        )?;
        fake.process(
            900,
            &["/usr/bin/crown-androidd"],
            1,
            "/user.slice/crown-androidd.service",
        )?;
        fake.process(901, &["/usr/bin/foot"], 1, "/user.slice/foot.scope")?;
        let processes = Processes::scan(&fake.host());
        assert_eq!(processes.of(Role::LxcStart), [Pid(710)]);
        assert_eq!(processes.of(Role::Daemon), [Pid(900)]);
        assert_eq!(processes.iter().count(), 3);
        assert_eq!(processes.container().len(), 2);
        Ok(())
    }
}
