#!/usr/bin/env python3
"""Cold boot: `waydroid session start` until sys.boot_completed=1. Runs as the desktop user.

Needs waydroid-container.service running and no session. For a cold page cache run
`root-steps.sh drop-caches` first. boot_progress_* events come from `root-steps.sh boot-events`,
which takes this script's t0 values.
"""

import argparse
import time

import common

BINDER_LOG_PATTERN = "binder"


def boot_completed_after(t0_ms: float, timeout_s: float) -> float | None:
    if common.wait_for(lambda: common.waydroid_prop("sys.boot_completed") == "1", timeout_s, 0.5) is None:
        return None
    return round(common.monotonic_ms() - t0_ms, 1)


def kernel_binder_lines(since_epoch_s: float) -> dict:
    journal = common.run(["journalctl", "-k", "-o", "short-monotonic", "--no-pager", "-g", BINDER_LOG_PATTERN,
                          "--since", f"@{since_epoch_s:.0f}"])
    if journal.returncode not in (0, 1):
        return {"error": journal.stderr.strip()[:200]}
    lines = [line for line in journal.stdout.splitlines() if not line.startswith("--")]
    return {"count": len(lines), "first": lines[:5], "enospc": sum("ENOSPC" in line for line in lines)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--label", default="cold")
    args = parser.parse_args()

    if common.session_running():
        common.emit("boot", {"error": "a session is already running; stop it first"})
        raise SystemExit(2)

    epoch_start = time.time()
    before = common.meminfo_kb()
    session = common.start_session(common.RESULTS_DIR / f"session-{args.label}.log", args.timeout)
    completed_ms = None
    if "ready" in session["milestones_ms"]:
        completed_ms = boot_completed_after(session["t0_monotonic_ms"], args.timeout)
    payload, monitor = common.container_cgroups()
    common.emit("boot", {
        "label": args.label,
        **session,
        "boot_completed_ms": completed_ms,
        "boot_completed_resolution_ms": 1000,
        "payload_cgroup": str(payload) if payload else None,
        "monitor_cgroup": str(monitor) if monitor else None,
        "meminfo_kb": {"before": before, "booted": common.meminfo_kb()},
        "kernel_binder_log": kernel_binder_lines(epoch_start),
    })


if __name__ == "__main__":
    main()
