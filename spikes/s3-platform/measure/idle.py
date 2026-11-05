#!/usr/bin/env python3
"""Idle CPU and wakeups of the running Android and the host-side Waydroid processes.

As the user: cgroup cpu.stat and per-thread context switches (a wakeup proxy).
As root (`root-steps.sh idle`): also counts sched_wakeup events whose target is an Android thread, via perf.
"""

import argparse
import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

import common

CLOCK_TICKS = os.sysconf("SC_CLK_TCK")
WAKEE = re.compile(rb"\bpid=(\d+)")


def thread_switches(pids: list[int]) -> dict[int, int]:
    switches = {}
    for pid in pids:
        for task in Path(f"/proc/{pid}/task").glob("*"):
            status = common.read_pairs(task / "status")
            if "voluntary_ctxt_switches" in status:
                switches[int(task.name)] = status["voluntary_ctxt_switches"] + status["nonvoluntary_ctxt_switches"]
    return switches


def cpu_ticks(pid: int) -> int | None:
    stat = common.read_text(f"/proc/{pid}/stat")
    if not stat:
        return None
    fields = stat.rsplit(")", 1)[1].split()
    return int(fields[11]) + int(fields[12])


def perf_wakeups(seconds: float, threads: set[int]) -> dict:
    with tempfile.TemporaryDirectory() as scratch:
        data = Path(scratch) / "perf.data"
        record = common.run(["perf", "record", "-q", "-a", "-e", "sched:sched_wakeup", "-o", str(data),
                             "--", "sleep", str(seconds)], timeout_s=seconds + 60)
        if record.returncode != 0:
            return {"error": record.stderr.strip()[:300]}
        total = android = 0
        with subprocess.Popen(["perf", "script", "-i", str(data), "-F", "trace"], stdout=subprocess.PIPE,
                              stderr=subprocess.DEVNULL) as trace:
            for line in trace.stdout:
                if match := WAKEE.search(line):
                    total += 1
                    android += int(match[1]) in threads
    return {"android_wakeups": android, "android_wakeups_per_s": round(android / seconds, 1),
            "system_wakeups_per_s": round(total / seconds, 1)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--label", default="idle-home")
    args = parser.parse_args()

    payload, monitor = common.container_cgroups()
    if payload is None:
        common.emit("idle", {"label": args.label, "error": "Android is not running"})
        raise SystemExit(1)
    host = {pid: role for role, pids in common.waydroid_processes().items() for pid in pids}
    pids = common.cgroup_pids(payload)
    switches_before = thread_switches(pids)
    usage_before = {"payload": common.cpu_usage_usec(payload), "monitor": common.cpu_usage_usec(monitor)}
    ticks_before = {pid: cpu_ticks(pid) for pid in host}
    started = time.monotonic()

    use_perf = os.geteuid() == 0 and shutil.which("perf")
    perf = perf_wakeups(args.seconds, set(switches_before)) if use_perf else None
    if not use_perf:
        time.sleep(args.seconds)

    elapsed = time.monotonic() - started
    switches_after = thread_switches(pids)
    common_threads = switches_before.keys() & switches_after.keys()
    switches = sum(switches_after[tid] - switches_before[tid] for tid in common_threads)
    def percent(usec: int) -> float:
        return round(usec / (elapsed * 1e6) * 100, 2)

    host_cpu = {}
    for pid, role in host.items():
        if (after := cpu_ticks(pid)) is not None and ticks_before[pid] is not None:
            host_cpu[f"{role}:{pid}"] = round((after - ticks_before[pid]) / CLOCK_TICKS / elapsed * 100, 2)
    common.emit("idle", {
        "label": args.label,
        "seconds": round(elapsed, 1),
        "frozen": common.is_frozen(payload),
        "payload_cgroup": str(payload),
        "payload_cpu_percent_of_one_core": percent(common.cpu_usage_usec(payload) - usage_before["payload"]),
        "monitor_cpu_percent_of_one_core": percent(common.cpu_usage_usec(monitor) - usage_before["monitor"])
        if monitor and usage_before["monitor"] is not None else None,
        "host_process_cpu_percent": host_cpu,
        "threads": {"tracked": len(common_threads), "appeared": len(switches_after.keys() - switches_before.keys()),
                    "exited": len(switches_before.keys() - switches_after.keys())},
        "context_switches_per_s": round(switches / elapsed, 1),
        "perf": perf,
    })


if __name__ == "__main__":
    main()
