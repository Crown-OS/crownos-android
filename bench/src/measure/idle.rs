use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::BufReader;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{NEEDS_ROOT, NOT_RUNNING};
use crate::findings::{Findings, MetricId, Report};
use crate::host::container::{self, ContainerCgroups};
use crate::host::processes::{Processes, Role};
use crate::host::{Host, Pid, Privilege};
use crate::parse::perf_trace::{self, WakeupTally};
use crate::parse::proc_stat::cpu_ticks;
use crate::parse::proc_status::context_switches;
use crate::unit::Unit;
use crate::util::command;
use crate::util::number::{hundredths, percent};

pub const ANDROID_CPU: MetricId = MetricId {
    name: "idle.android_cpu",
    unit: Unit::Percent,
};
pub const ANDROID_WAKEUPS: MetricId = MetricId {
    name: "idle.android_wakeups",
    unit: Unit::PerSecond,
};
pub const DAEMON_CPU: MetricId = MetricId {
    name: "idle.daemon_cpu",
    unit: Unit::Percent,
};

/// Time perf gets beyond the sample to start and write its data.
const PERF_GRACE: Duration = Duration::from_secs(60);
const PERF_DATA: &str = "crown-android-bench-perf.data";
const MICROSECONDS_PER_SECOND: f64 = 1_000_000.0;

/// Android's threads over the sample.
#[derive(Debug, Serialize)]
pub struct Threads {
    pub tracked: usize,
    pub appeared: usize,
    pub exited: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Wakeups {
    Counted {
        android: u64,
        android_per_second: f64,
        system_per_second: f64,
    },
    Skipped(String),
}

/// CPU use and wakeups while Android sits idle, as S3's `idle.py` samples them.
#[derive(Debug, Serialize)]
pub struct Idle {
    pub seconds: f64,
    pub frozen: Option<bool>,
    pub payload_cpu_percent: f64,
    pub monitor_cpu_percent: Option<f64>,
    /// Host processes by `role:pid`, in percent of one core.
    pub host_cpu_percent: BTreeMap<String, f64>,
    pub daemon_cpu_percent: Option<f64>,
    pub threads: Threads,
    /// Voluntary and involuntary switches of Android's threads, a wakeup proxy that needs no root.
    pub context_switches_per_second: f64,
    pub wakeups: Wakeups,
}

pub fn measure(
    host: &Host,
    privilege: Privilege,
    processes: &Processes,
    cgroups: &ContainerCgroups,
    sample: Duration,
    scratch: &Path,
) -> Result<Idle> {
    let payload = cgroups.payload.as_deref().context(NOT_RUNNING)?;
    let unreadable = "cpu.stat of the payload cgroup is unreadable";
    let android = container::members(payload);
    let switches_before = thread_switches(host, &android);
    let payload_before = container::cpu_usage_usec(payload).context(unreadable)?;
    let monitor_before = cgroups
        .monitor
        .as_deref()
        .and_then(container::cpu_usage_usec);
    let ticks_before: Vec<(Role, Pid, Option<u64>)> = processes
        .iter()
        .map(|(role, pid)| (role, pid, ticks(host, pid)))
        .collect();
    let started = Instant::now();
    let tally = match privilege {
        Privilege::Root => {
            let threads = switches_before.keys().map(|thread| thread.0).collect();
            perf_wakeups(sample, &threads, scratch).map_err(|error| format!("{error:#}"))
        }
        Privilege::User => Err(NEEDS_ROOT.to_owned()),
    };
    thread::sleep(sample.saturating_sub(started.elapsed()));
    let elapsed = started.elapsed().as_secs_f64();

    let switches_after = thread_switches(host, &android);
    let switches: u64 = switches_after
        .iter()
        .filter_map(|(thread, after)| Some(after.saturating_sub(*switches_before.get(thread)?)))
        .sum();
    let payload_used = container::cpu_usage_usec(payload)
        .context(unreadable)?
        .saturating_sub(payload_before);
    let monitor_used = cgroups
        .monitor
        .as_deref()
        .and_then(container::cpu_usage_usec)
        .zip(monitor_before)
        .map(|(after, before)| after.saturating_sub(before));
    let cpu_percent =
        |microseconds: u64| percent(microseconds as f64 / MICROSECONDS_PER_SECOND, elapsed);
    let ticks_per_second = rustix::param::clock_ticks_per_second() as f64;
    let mut host_cpu_percent = BTreeMap::new();
    let mut daemon_cpu_percent = None;
    for (role, pid, before) in ticks_before {
        let Some(used) = before
            .zip(ticks(host, pid))
            .map(|(before, after)| after.saturating_sub(before))
        else {
            continue;
        };
        let share = percent(used as f64 / ticks_per_second, elapsed);
        if role == Role::Daemon {
            daemon_cpu_percent = Some(daemon_cpu_percent.unwrap_or(0.0) + share);
        }
        host_cpu_percent.insert(format!("{}:{pid}", role.name()), share);
    }
    let common = switches_after
        .keys()
        .filter(|thread| switches_before.contains_key(thread))
        .count();
    Ok(Idle {
        seconds: hundredths(elapsed),
        frozen: container::is_frozen(payload),
        payload_cpu_percent: cpu_percent(payload_used),
        monitor_cpu_percent: monitor_used.map(cpu_percent),
        host_cpu_percent,
        daemon_cpu_percent,
        threads: Threads {
            tracked: common,
            appeared: switches_after.len() - common,
            exited: switches_before.len() - common,
        },
        context_switches_per_second: hundredths(switches as f64 / elapsed),
        wakeups: match tally {
            Ok(tally) => Wakeups::Counted {
                android: tally.tracked,
                android_per_second: hundredths(tally.tracked as f64 / elapsed),
                system_per_second: hundredths(tally.total as f64 / elapsed),
            },
            Err(reason) => Wakeups::Skipped(reason),
        },
    })
}

fn ticks(host: &Host, pid: Pid) -> Option<u64> {
    cpu_ticks(&host.read_process(pid, "stat")?)
}

fn thread_switches(host: &Host, pids: &[Pid]) -> HashMap<Pid, u64> {
    pids.iter()
        .flat_map(|&pid| {
            host.threads(pid)
                .into_iter()
                .map(move |thread| (pid, thread))
        })
        .filter_map(|(pid, thread)| {
            Some((thread, context_switches(&host.thread_status(pid, thread)?)?))
        })
        .collect()
}

/// Records `sched:sched_wakeup` system-wide for the sample, as `idle.py` does as root,
/// and counts the events that woke one of `threads`.
fn perf_wakeups(sample: Duration, threads: &HashSet<u32>, scratch: &Path) -> Result<WakeupTally> {
    let data = scratch.join(PERF_DATA);
    let data_path = data
        .to_str()
        .context("the results directory is not UTF-8")?;
    let seconds = sample.as_secs().to_string();
    let record = command::run(
        "perf",
        &[
            "record",
            "-q",
            "-a",
            "-e",
            "sched:sched_wakeup",
            "-o",
            data_path,
            "--",
            "sleep",
            &seconds,
        ],
        sample + PERF_GRACE,
    )?;
    if !record.success() {
        bail!("perf record failed: {}", record.error());
    }
    let tally = count_wakeups(&data, threads);
    std::fs::remove_file(&data).ok();
    tally
}

fn count_wakeups(data: &Path, threads: &HashSet<u32>) -> Result<WakeupTally> {
    let mut script = Command::new("perf")
        .arg("script")
        .arg("-i")
        .arg(data)
        .args(["-F", "trace"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("could not start perf script")?;
    let trace = script.stdout.take().context("perf script has no output")?;
    let tally = perf_trace::tally(BufReader::new(trace), threads);
    script.wait()?;
    Ok(tally?)
}

impl Report for Idle {
    fn report(&self, _step: &str, findings: &mut Findings) {
        findings.metric(ANDROID_CPU, self.payload_cpu_percent);
        match &self.wakeups {
            Wakeups::Counted {
                android_per_second, ..
            } => findings.metric(ANDROID_WAKEUPS, *android_per_second),
            Wakeups::Skipped(reason) => findings.skip(ANDROID_WAKEUPS, reason.as_str()),
        }
        match self.daemon_cpu_percent {
            Some(share) => findings.metric(DAEMON_CPU, share),
            None => findings.skip(DAEMON_CPU, "crown-androidd is not running"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::FakeHost;

    #[test]
    fn samples_the_payload_without_root() -> anyhow::Result<()> {
        let fake = FakeHost::new("idle")?;
        fake.process(
            710,
            &["lxc-start", "-n", "waydroid"],
            1,
            "/lxc.monitor.waydroid",
        )?;
        fake.process(711, &["/init"], 710, "/lxc.payload.waydroid")?;
        fake.process(
            950,
            &["/usr/bin/crown-androidd"],
            1,
            "/user.slice/crown-androidd.service",
        )?;
        fake.file("proc/950/stat", include_str!("../../fixtures/stat"))?;
        fake.file(
            "proc/711/task/711/status",
            include_str!("../../fixtures/status"),
        )?;
        fake.file(
            "sys/fs/cgroup/lxc.payload.waydroid/cpu.stat",
            include_str!("../../fixtures/cpu.stat"),
        )?;
        let host = fake.host();
        let processes = Processes::scan(&host);
        let cgroups = ContainerCgroups::find(&host, &processes);
        let idle = measure(
            &host,
            Privilege::User,
            &processes,
            &cgroups,
            Duration::from_millis(20),
            &fake.path(""),
        )?;
        assert_eq!(idle.payload_cpu_percent, 0.0);
        assert_eq!(idle.daemon_cpu_percent, Some(0.0));
        assert_eq!(idle.threads.tracked, 1);
        assert!(matches!(&idle.wakeups, Wakeups::Skipped(reason) if reason == NEEDS_ROOT));
        assert!(idle.host_cpu_percent.contains_key("daemon:950"));
        let mut findings = Findings::default();
        idle.report("idle", &mut findings);
        assert_eq!(findings.metrics.len(), 2);
        assert_eq!(findings.skipped[ANDROID_WAKEUPS.name], NEEDS_ROOT);
        assert!(
            measure(
                &host,
                Privilege::User,
                &processes,
                &ContainerCgroups::default(),
                Duration::ZERO,
                &fake.path("")
            )
            .is_err()
        );
        Ok(())
    }
}
