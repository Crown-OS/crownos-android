"""Shared probes for the S3 measurement scripts. Standard library only."""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

CGROUP_ROOT = Path("/sys/fs/cgroup")
WAYDROID_ROOTFS = "/var/lib/waydroid/rootfs"
READY_LINE = "Android with user 0 is ready"
RESULTS_DIR = Path.home() / ".cache/crownos-android/s3"
MEMINFO_KEYS = ("MemTotal", "MemFree", "MemAvailable", "Cached", "Shmem", "AnonPages",
                "SwapTotal", "SwapFree", "Zswap", "Zswapped")
ROLLUP_KEYS = ("Rss", "Pss", "Pss_Anon", "Pss_File", "Pss_Shmem", "Swap", "SwapPss")
CGROUP_SCALARS = ("memory.current", "memory.peak", "memory.swap.current", "memory.zswap.current")
BUSCTL_CONTAINER = ["busctl", "--system", "call", "id.waydro.Container",
                    "/ContainerManager", "id.waydro.ContainerManager"]
SCAN_INTERVAL_S = 0.25
POLL_INTERVAL_S = 0.05


def monotonic_ms() -> float:
    return time.clock_gettime_ns(time.CLOCK_MONOTONIC) / 1e6


def boottime_ms() -> float:
    return time.clock_gettime_ns(time.CLOCK_BOOTTIME) / 1e6


def emit(step: str, result: dict) -> None:
    json.dump({"step": step, "euid": os.geteuid(), **result}, sys.stdout, separators=(",", ":"))
    sys.stdout.write("\n")


def read_text(path) -> str | None:
    try:
        return Path(path).read_text()
    except OSError:
        return None


def parse_pairs(text: str) -> dict[str, int]:
    pairs = {}
    for line in text.splitlines():
        fields = line.replace(":", " ").split()
        if len(fields) >= 2 and fields[1].isdigit():
            pairs[fields[0]] = int(fields[1])
    return pairs


def read_pairs(path) -> dict[str, int]:
    return parse_pairs(read_text(path) or "")


def meminfo_kb() -> dict[str, int]:
    info = read_pairs("/proc/meminfo")
    return {key: info[key] for key in MEMINFO_KEYS if key in info}


def memory_pressure() -> dict[str, float]:
    pressure = {}
    for line in (read_text("/proc/pressure/memory") or "").splitlines():
        kind, *fields = line.split()
        pressure.update({f"{kind}_{k}": float(v) for k, v in (f.split("=") for f in fields) if k != "total"})
    return pressure


def argv_of(pid: int) -> list[str]:
    return [part for part in (read_text(f"/proc/{pid}/cmdline") or "").split("\0") if part]


def all_pids() -> list[int]:
    return [int(entry) for entry in os.listdir("/proc") if entry.isdigit()]


def role_of(argv: list[str]) -> str | None:
    joined = " ".join(argv)
    program = argv[0] if argv else ""
    if "waydroid container start" in joined:
        return "container_manager"
    if "waydroid session start" in joined:
        return "session_manager"
    if program.endswith("lxc-start") and "waydroid" in argv:
        return "lxc_start"
    if program.endswith("dnsmasq") and "/run/waydroid-lxc/" in joined:
        return "dnsmasq"
    if program.endswith("waydroid-sensord"):
        return "sensord"
    return None


def waydroid_processes() -> dict[str, list[int]]:
    found = {"container_manager": [], "session_manager": [], "lxc_start": [], "dnsmasq": [], "sensord": []}
    for pid in all_pids():
        if role := role_of(argv_of(pid)):
            found[role].append(pid)
    return found


def cgroup_of(pid: int) -> Path | None:
    for line in (read_text(f"/proc/{pid}/cgroup") or "").splitlines():
        if line.startswith("0::"):
            return CGROUP_ROOT / line[3:].lstrip("/")
    return None


def container_cgroups(monitor: int | None = None) -> tuple[Path | None, Path | None]:
    """(payload, monitor): the cgroup of lxc-start's container init, and lxc-start's own."""
    if monitor is None:
        monitors = waydroid_processes()["lxc_start"]
        if not monitors:
            return None, None
        monitor = monitors[0]
    monitor_cgroup = cgroup_of(monitor)
    for pid in all_pids():
        if read_pairs(f"/proc/{pid}/status").get("PPid") == monitor:
            if (cgroup := cgroup_of(pid)) and cgroup != monitor_cgroup:
                return cgroup, monitor_cgroup
    return None, monitor_cgroup


def cgroup_pids(cgroup: Path) -> list[int]:
    return [int(pid) for procs in cgroup.rglob("cgroup.procs") for pid in (read_text(procs) or "").split()]


def cgroup_memory(cgroup: Path | None) -> dict | None:
    if cgroup is None:
        return None
    scalars = {name: int(text) for name in CGROUP_SCALARS
               if (text := (read_text(cgroup / name) or "").strip()).isdigit()}
    return {"path": str(cgroup), **scalars, "memory.stat": read_pairs(cgroup / "memory.stat")}


def cpu_usage_usec(cgroup: Path | None) -> int | None:
    return read_pairs(cgroup / "cpu.stat").get("usage_usec") if cgroup else None


def is_frozen(cgroup: Path) -> bool | None:
    events = read_pairs(cgroup / "cgroup.events")
    return bool(events["frozen"]) if "frozen" in events else None


def smaps_rollup(pid: int) -> dict:
    try:
        pairs = parse_pairs(Path(f"/proc/{pid}/smaps_rollup").read_text())
    except PermissionError:
        return {"pid": pid, "error": "needs root"}
    except OSError as error:
        return {"pid": pid, "error": error.strerror}
    return {"pid": pid, **{f"{key}_kb": pairs[key] for key in ROLLUP_KEYS if key in pairs}}


def wait_for(predicate, timeout_s: float, interval_s: float = POLL_INTERVAL_S) -> float | None:
    """Milliseconds until predicate() is true, or None on timeout."""
    start = monotonic_ms()
    while monotonic_ms() - start < timeout_s * 1000:
        if predicate():
            return round(monotonic_ms() - start, 1)
        time.sleep(interval_s)
    return None


def run(argv: list[str], timeout_s: float = 60) -> subprocess.CompletedProcess:
    return subprocess.run(argv, capture_output=True, text=True, timeout=timeout_s, check=False)


def waydroid_prop(key: str) -> str:
    return run(["waydroid", "prop", "get", key]).stdout.strip()


def waydroid_mounts() -> int:
    return sum(WAYDROID_ROOTFS in line for line in (read_text("/proc/self/mountinfo") or "").splitlines())


def session_running() -> bool:
    return bool(waydroid_processes()["session_manager"])


def start_session(log_path: Path, timeout_s: float) -> dict:
    """Starts `waydroid session start` detached and times it until Android unlocks user 0."""
    log_path.parent.mkdir(parents=True, exist_ok=True)
    t0, t0_boot = monotonic_ms(), boottime_ms()
    with log_path.open("w") as log:
        session = subprocess.Popen(["waydroid", "session", "start"], stdout=log, stderr=subprocess.STDOUT,
                                   stdin=subprocess.DEVNULL, start_new_session=True)
    milestones, next_scan = {}, 0.0
    while monotonic_ms() - t0 < timeout_s * 1000 and session.poll() is None:
        if READY_LINE in (read_text(log_path) or ""):
            milestones["ready"] = round(monotonic_ms() - t0, 1)
            break
        if "payload_cgroup" not in milestones and monotonic_ms() >= next_scan:
            next_scan = monotonic_ms() + SCAN_INTERVAL_S * 1000
            if container_cgroups()[0]:
                milestones["payload_cgroup"] = round(monotonic_ms() - t0, 1)
        time.sleep(POLL_INTERVAL_S)
    return {"t0_monotonic_ms": round(t0, 1), "t0_boottime_ms": round(t0_boot, 1), "session_pid": session.pid,
            "session_exit_code": session.poll(), "milestones_ms": milestones,
            "payload_cgroup_resolution_ms": SCAN_INTERVAL_S * 1000, "log": str(log_path)}


def stop_session(timeout_s: float) -> dict:
    monitors = waydroid_processes()["lxc_start"]
    payload, _ = container_cgroups(monitors[0]) if monitors else (None, None)
    t0 = monotonic_ms()
    call = run(["waydroid", "session", "stop"], timeout_s)
    call_ms = round(monotonic_ms() - t0, 1)

    def elapsed_once(predicate) -> float | None:
        return None if wait_for(predicate, timeout_s) is None else round(monotonic_ms() - t0, 1)

    container_gone_ms = elapsed_once(lambda: not (payload and payload.exists())
                                     and not any(Path(f"/proc/{pid}").exists() for pid in monitors))
    rootfs_unmounted_ms = elapsed_once(lambda: waydroid_mounts() == 0)
    return {"call_ms": call_ms, "exit_code": call.returncode, "payload_cgroup": str(payload) if payload else None,
            "container_gone_ms": container_gone_ms, "rootfs_unmounted_ms": rootfs_unmounted_ms}
