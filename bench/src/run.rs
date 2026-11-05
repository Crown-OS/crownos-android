use std::path::Path;
use std::thread;

use crate::cli::Args;
use crate::findings::Findings;
use crate::host::container::ContainerCgroups;
use crate::host::processes::Processes;
use crate::host::{Host, Privilege};
use crate::measure::launch::{self, ColdStart, LaunchTimes, Launches};
use crate::measure::lifecycle::{self, BudgetedStop, Restart};
use crate::measure::{cgroup_memory, idle, leftovers, mem_available, pss};
use crate::wayland::toplevels::Toplevels;

/// One full run, in the order of S3's `run-baseline.sh`. A failed step is
/// recorded and the run goes on with whatever still applies.
pub fn run(args: &Args, host: &Host, privilege: Privilege, scratch: &Path) -> Findings {
    let mut findings = Findings::default();
    let (mut toplevels, unwatched) = match Toplevels::connect() {
        Ok(toplevels) => (Some(toplevels), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };

    if lifecycle::is_running(host) {
        findings.record("initial_stop", lifecycle::stop(host, args.timeout()));
    }
    thread::sleep(args.after_stop_settle());
    let reference = match mem_available::reference(host) {
        Ok(kib) => Some(kib),
        Err(error) => {
            findings.fail("mem_available", format!("{error:#}"));
            None
        }
    };

    let launches = launch_apps(args, host, toplevels.as_mut(), &mut findings);
    findings.record(
        "launch",
        Ok(Launches {
            unwatched,
            launches,
        }),
    );
    thread::sleep(args.settle());

    let processes = Processes::scan(host);
    let cgroups = ContainerCgroups::find(host, &processes);
    let android_apps = toplevels
        .as_mut()
        .and_then(|toplevels| toplevels.sync().ok().map(|()| toplevels.android_apps()));
    findings.record(
        "memory",
        cgroup_memory::measure(host, &cgroups, android_apps),
    );
    findings.record(
        "pss",
        Ok(pss::measure(host, privilege, &processes, &cgroups)),
    );
    let idle = idle::measure(host, privilege, &processes, &cgroups, args.idle(), scratch);
    findings.record("idle", idle);
    findings.record(
        "freeze",
        lifecycle::freeze_cycle(host, &cgroups, args.frozen_hold()),
    );

    findings.record(
        "stop",
        lifecycle::stop(host, args.timeout()).map(BudgetedStop),
    );
    thread::sleep(args.after_stop_settle());
    if let Some(reference) = reference {
        findings.record("mem_available", mem_available::after_stop(host, reference));
    }
    findings.record(
        "leftovers",
        Ok(leftovers::measure(host, cgroups.payload.as_deref())),
    );

    if let Some(package) = args.apps.first() {
        let restart = launch::start_android(host, toplevels.as_mut(), package, args.timeout());
        findings.record("restart", restart.map(Restart));
        findings.record("final_stop", lifecycle::stop(host, args.timeout()));
    }
    findings
}

/// The first app starts Android; the others launch while it runs.
fn launch_apps(
    args: &Args,
    host: &Host,
    mut toplevels: Option<&mut Toplevels>,
    findings: &mut Findings,
) -> Vec<LaunchTimes> {
    let Some((first, others)) = args.apps.split_first() else {
        return Vec::new();
    };
    let mut launches = Vec::with_capacity(args.apps.len());
    match launch::start_android(host, toplevels.as_deref_mut(), first, args.timeout()) {
        Ok(ColdStart { boot, launch }) => {
            findings.record("boot", Ok(boot));
            launches.push(launch);
        }
        Err(error) => {
            findings.fail("boot", format!("{error:#}"));
            return launches;
        }
    }
    for package in others {
        match launch::warm(toplevels.as_deref_mut(), package, args.timeout()) {
            Ok(launch) => launches.push(launch),
            Err(error) => findings.fail(&format!("launch {package}"), format!("{error:#}")),
        }
    }
    launches
}
