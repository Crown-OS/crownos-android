#!/usr/bin/env bash
# The S3 baseline in order. Run as the desktop user, inside the graphical session, booted into
# the kernel under test (linux-cachyos for the ISO numbers: zen's Rust binder hits #2422).
# Root steps go through `sudo root-steps.sh`; --no-root skips them.
#
#   run-baseline.sh [--no-root] [--app PACKAGE]
#
# Every step writes one JSON object to the results directory; the last line printed is one
# JSON object with all of them, keyed by step and label.
#
#   step                      runs as  what
#   service-stop/-start       root     waydroid-container.service, MemAvailable before it starts
#   drop-caches               root     cold page cache
#   boot (cold)               user     session start -> container cgroup -> user 0 ready -> boot_completed
#   boot_events               root     boot_progress_* and ro.boottime.* relative to boot's t0
#   binder_check              root     driver, binderfs, nodes, kernel binder errors
#   memory                    user     cgroup memory.current/memory.stat, session manager PSS
#   memory (root)             root     container manager, lxc-start, dnsmasq PSS, Android PSS sum
#   android_meminfo           root     dumpsys meminfo top entries
#   idle                      user     cgroup CPU %, context switches/s
#   idle (root)               root     Android wakeups/s through perf
#   lifecycle                 user     freeze/unfreeze, stop, warm start, MemAvailable, leftovers
#   full_stop                 user     MemAvailable after the service stops, against the pre-start value

set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root_steps=$here/../root-steps.sh
out=${S3_RESULTS:-$HOME/.cache/crownos-android/s3/results/$(date +%Y%m%dT%H%M%S)}
settle_seconds=${SETTLE_SECONDS:-60}
with_root=1 app=''

while (($#)); do
    case $1 in
        --no-root) with_root=0 ;;
        --app) app=${2:?package name}; shift ;;
        *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

mem_available_kb() { awk '/^MemAvailable:/ {print $2}' /proc/meminfo; }
as_root() { ((with_root)) || return 0; sudo -- "$root_steps" "$@"; }
record() {
    local name=$1
    shift
    "$@" >"$out/$name.json" || printf '{"step":"%s","error":"exit %d"}\n' "$name" "$?" >"$out/$name.json"
}
measure_state() {
    local label=$1
    record "memory-$label" python3 "$here/memory.py" --label "$label"
    record "memory-root-$label" as_root memory --label "$label"
    record "meminfo-$label" as_root meminfo --label "$label"
    record "idle-$label" python3 "$here/idle.py" --seconds 60 --label "$label"
    record "idle-root-$label" as_root idle --label "$label"
}

mkdir -p -- "$out"
"$here/../kernel-check.sh" /proc/config.gz >"$out/kernel.md"
waydroid session stop >/dev/null 2>&1 || true

record service-stop-initial as_root service-stop
sleep 10
baseline_kb=$(mem_available_kb)
record drop-caches as_root drop-caches
record service-start as_root service-start

record boot-cold python3 "$here/boot.py" --label cold
record boot-events as_root boot-events \
    "$(jq -r '.t0_monotonic_ms // empty' "$out/boot-cold.json")" "$(jq -r '.t0_boottime_ms // empty' "$out/boot-cold.json")"
record binder as_root binder-check
record stay-awake as_root stay-awake

sleep "$settle_seconds"
measure_state booted
if [[ -n $app ]]; then
    waydroid app launch "$app"
    sleep "$settle_seconds"
    measure_state "app-${app##*.}"
fi

record lifecycle python3 "$here/lifecycle.py" --baseline-kb "$baseline_kb"
record service-stop as_root service-stop
sleep 10
jq -n --argjson before "$baseline_kb" --argjson after "$(mem_available_kb)" \
    '{step: "full_stop", mem_available_kb: {before_service: $before, after_service_stop: $after,
      delta: ($after - $before), budget_met: (($before - $after) <= 51200)}}' >"$out/full-stop.json"

printf 'results: %s\n' "$out" >&2
jq -cs 'map(select(.step) | {([.step, .label, (if .euid == 0 then "root" else empty end)]
    | map(select(.)) | join("/")): .}) | add' "$out"/*.json
