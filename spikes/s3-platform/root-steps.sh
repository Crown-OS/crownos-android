#!/usr/bin/env bash
# Every S3 step that needs root, one subcommand each. measure/run-baseline.sh calls the
# measurement ones through sudo; nothing here runs unless you invoke it.
#
#   sudo root-steps.sh service-start | service-stop     waydroid-container.service, timed
#   sudo root-steps.sh drop-caches                      cold page cache before a cold boot
#   sudo root-steps.sh stay-awake                       keep Android's display on, so Waydroid
#                                                       does not freeze it during idle sampling
#   sudo root-steps.sh boot-events T0_MONO T0_BOOT      boot_progress_* and ro.boottime.*
#   sudo root-steps.sh binder-check                     binder facts plus kernel binder errors
#   sudo root-steps.sh memory [--label L]               PSS of root-owned helpers and Android
#   sudo root-steps.sh meminfo [--label L]              dumpsys meminfo top entries
#   sudo root-steps.sh idle [--seconds N] [--label L]   exact Android wakeups through perf
#
# Optional and mutating (prototype of the crownos-android-setup binder step, not part of the
# baseline): pins Waydroid to binder/vndbinder/hwbinder on a binderfs mount, on any kernel.
#   sudo root-steps.sh binder-nodes

set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
measure=$here/measure
canonical_nodes=(binder vndbinder hwbinder)

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
py() { python3 -E -s "$measure/$1.py" "${@:2}"; }
now_ms() { local ns; ns=$(date +%s%N); printf '%s' "$((ns / 1000000))"; }
mem_available_kb() { awk '/^MemAvailable:/ {print $2}' /proc/meminfo; }

timed_service() {
    local action=$1 start
    start=$(now_ms)
    systemctl "$action" waydroid-container.service
    printf '{"step":"service-%s","ms":%d,"mem_available_kb":%d}\n' \
        "$action" "$(($(now_ms) - start))" "$(mem_available_kb)"
}

drop_caches() {
    sync
    echo 3 >/proc/sys/vm/drop_caches
    printf '{"step":"drop-caches","mem_available_kb":%d}\n' "$(mem_available_kb)"
}

stay_awake() {
    waydroid shell -- svc power stayon true
    printf '{"step":"stay-awake","ok":true}\n'
}

binder_nodes() {
    mkdir -p /dev/binderfs
    mountpoint -q /dev/binderfs || mount -t binder binder /dev/binderfs
    python3 -E -s -c '
import fcntl, os, struct, sys
BINDER_CTL_ADD = 0xC1086201
with open("/dev/binderfs/binder-control", "rb") as control:
    for name in sys.argv[1:]:
        if not os.path.exists("/dev/binderfs/" + name):
            fcntl.ioctl(control, BINDER_CTL_ADD, struct.pack("256sII", name.encode(), 0, 0))
' "${canonical_nodes[@]}"
    local name
    for name in "${canonical_nodes[@]}"; do
        chmod 666 "/dev/binderfs/$name"
        ln -sfn "/dev/binderfs/$name" "/dev/$name"
    done
    sed -i -E 's/^(binder|vndbinder|hwbinder) = .*/\1 = \1/' /var/lib/waydroid/waydroid.cfg
    waydroid upgrade --offline >&2
    py binder_check
}

((EUID == 0)) || die "run with sudo"
command=${1:-}
shift || true
case $command in
    service-start) timed_service start ;;
    service-stop) timed_service stop ;;
    drop-caches) drop_caches ;;
    stay-awake) stay_awake ;;
    boot-events) py boot_events "$@" ;;
    binder-check) py binder_check ;;
    memory) py memory "$@" ;;
    meminfo) py android_meminfo "$@" ;;
    idle) py idle --seconds 30 "$@" ;;
    binder-nodes) binder_nodes ;;
    *) sed -n '2,18p' "${BASH_SOURCE[0]}" >&2; exit 2 ;;
esac
