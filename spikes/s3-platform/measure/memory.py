#!/usr/bin/env python3
"""Memory per component while Android runs.

As the user: cgroup memory.current/memory.stat (world-readable) and the session manager's PSS.
As root (`root-steps.sh memory`): also the root-owned container manager, lxc-start and dnsmasq,
and the PSS sum and top processes of the Android payload cgroup.
"""

import argparse
import os

import common

TOP_PROCESSES = 15


def comm_of(pid: int) -> str:
    return (common.read_text(f"/proc/{pid}/comm") or "?").strip()


def payload_pss(payload) -> dict:
    rollups = [rollup | {"comm": comm_of(rollup["pid"])}
               for rollup in map(common.smaps_rollup, common.cgroup_pids(payload)) if "Pss_kb" in rollup]
    rollups.sort(key=lambda rollup: rollup["Pss_kb"], reverse=True)
    return {"processes": len(rollups), "pss_kb": sum(r["Pss_kb"] for r in rollups),
            "swap_pss_kb": sum(r.get("SwapPss_kb", 0) for r in rollups), "top": rollups[:TOP_PROCESSES]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label", default="idle-home")
    args = parser.parse_args()

    payload, monitor = common.container_cgroups()
    host = {role: [common.smaps_rollup(pid) for pid in pids] for role, pids in common.waydroid_processes().items()}
    result = {
        "label": args.label,
        "meminfo_kb": common.meminfo_kb(),
        "memory_pressure": common.memory_pressure(),
        "payload": common.cgroup_memory(payload),
        "monitor": common.cgroup_memory(monitor),
        "host_processes": host,
    }
    if payload and os.geteuid() == 0:
        result["payload_pss"] = payload_pss(payload)
    common.emit("memory", result)


if __name__ == "__main__":
    main()
