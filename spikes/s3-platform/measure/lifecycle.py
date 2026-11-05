#!/usr/bin/env python3
"""Freeze/unfreeze, stop/start timings and MemAvailable across a full cycle. Runs as the desktop user.

Freeze and Unfreeze go through id.waydro.Container on the system bus, which Waydroid's policy
opens to every local user. Needs waydroid-container.service running. Pass --baseline-kb with
MemAvailable taken before the service started (run-baseline.sh does) to compare against a true
pre-start value; the full service stop itself is `root-steps.sh service-stop`.
"""

import argparse
import time

import common

FROZEN_TIMEOUT_S = 30


def toggle_freeze(payload, method: str, frozen: bool) -> dict:
    t0 = common.monotonic_ms()
    call = common.run([*common.BUSCTL_CONTAINER, method], FROZEN_TIMEOUT_S)
    call_ms = round(common.monotonic_ms() - t0, 1)
    settled = common.wait_for(lambda: common.is_frozen(payload) is frozen, FROZEN_TIMEOUT_S, 0.005)
    return {"call_ms": call_ms, "exit_code": call.returncode, "error": call.stderr.strip()[:200] or None,
            "cgroup_settled_after_call_ms": settled}


def frozen_cpu_usec(payload, seconds: float) -> int | None:
    before = common.cpu_usage_usec(payload)
    time.sleep(seconds)
    after = common.cpu_usage_usec(payload)
    return None if before is None or after is None else after - before


def leftovers() -> dict:
    processes = common.waydroid_processes()
    return {"rootfs_mounts": common.waydroid_mounts(),
            "lxc_start": processes["lxc_start"], "dnsmasq": processes["dnsmasq"],
            "session_manager": processes["session_manager"],
            "bridge_waydroid0": (common.read_text("/sys/class/net/waydroid0/operstate") or "").strip() or None}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-kb", type=int, help="MemAvailable before waydroid-container.service started")
    parser.add_argument("--settle", type=float, default=30, help="seconds to idle after boot before measuring")
    parser.add_argument("--after-stop-settle", type=float, default=10)
    parser.add_argument("--frozen-hold", type=float, default=5)
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()

    mem = {"baseline_kb": args.baseline_kb}
    first_start = None
    if not common.session_running():
        mem["before_start_kb"] = common.meminfo_kb()["MemAvailable"]
        first_start = common.start_session(common.RESULTS_DIR / "session-lifecycle.log", args.timeout)
    time.sleep(args.settle)
    payload, _ = common.container_cgroups()
    if payload is None:
        common.emit("lifecycle", {"error": "Android did not come up", "first_start": first_start})
        raise SystemExit(1)
    mem["running_kb"] = common.meminfo_kb()["MemAvailable"]
    running_cgroup = common.cgroup_memory(payload)

    freeze = toggle_freeze(payload, "Freeze", True)
    freeze["cpu_usec_while_frozen"] = frozen_cpu_usec(payload, args.frozen_hold)
    mem["frozen_kb"] = common.meminfo_kb()["MemAvailable"]
    unfreeze = toggle_freeze(payload, "Unfreeze", False)

    stop = common.stop_session(args.timeout)
    time.sleep(args.after_stop_settle)
    mem["after_stop_kb"] = common.meminfo_kb()["MemAvailable"]
    after_stop = leftovers()

    warm_start = common.start_session(common.RESULTS_DIR / "session-warm.log", args.timeout)
    warm_stop = common.stop_session(args.timeout)

    reference = args.baseline_kb or mem.get("before_start_kb")
    common.emit("lifecycle", {
        "first_start": first_start,
        "payload_memory_current_running": running_cgroup and running_cgroup.get("memory.current"),
        "freeze": freeze,
        "unfreeze": unfreeze,
        "stop": stop,
        "leftovers_after_stop": after_stop,
        "warm_start": warm_start,
        "warm_stop": warm_stop,
        "mem_available": mem,
        "after_stop_minus_reference_kb": None if reference is None else mem["after_stop_kb"] - reference,
    })


if __name__ == "__main__":
    main()
