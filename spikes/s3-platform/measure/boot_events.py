#!/usr/bin/env python3
"""boot_progress_* events and init boot times of the running Android. Needs root (`waydroid shell`).

Event values are SystemClock.uptimeMillis (host CLOCK_MONOTONIC); ro.boottime.* are CLOCK_BOOTTIME ns.
Pass boot.py's t0_monotonic_ms and t0_boottime_ms to get offsets from session start.
"""

import argparse
import re

import common

EVENT = re.compile(r"\s(boot_progress_\w+|sf_stop_bootanim|wm_boot_animation_done)\s*:\s*\[?(\d+)")
BOOTTIME_PROP = re.compile(r"^\[ro\.boottime\.([\w.-]+)\]: \[(\d+)\]$")


def android(command: list[str]) -> str:
    return common.run(["waydroid", "shell", "--", *command], timeout_s=120).stdout


def first_events() -> dict[str, int]:
    events = {}
    for match in EVENT.finditer(android(["logcat", "-b", "events", "-d"])):
        events.setdefault(match[1], int(match[2]))
    return events


def boottimes_ms() -> dict[str, float]:
    return {match[1]: int(match[2]) / 1e6
            for line in android(["getprop"]).splitlines() if (match := BOOTTIME_PROP.match(line))}


def offsets(values: dict, t0: float | None) -> dict | None:
    return None if t0 is None else {name: round(value - t0, 1) for name, value in values.items()}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("t0_monotonic_ms", type=float, nargs="?")
    parser.add_argument("t0_boottime_ms", type=float, nargs="?")
    args = parser.parse_args()

    events, boottimes = first_events(), boottimes_ms()
    common.emit("boot_events", {
        "events_uptime_ms": events,
        "events_since_t0_ms": offsets(events, args.t0_monotonic_ms),
        "boottime_ms": boottimes,
        "boottime_since_t0_ms": offsets(boottimes, args.t0_boottime_ms),
        "boot_completed": android(["getprop", "sys.boot_completed"]).strip(),
    })


if __name__ == "__main__":
    main()
