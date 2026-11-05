#!/usr/bin/env bash
# Wave 0 runtime runbook. Run it in your own terminal, inside your desktop session:
#
#   /e/CrownOS/crownos-android/spikes/run-wave0.sh            # s3, then s2, then s1
#   /e/CrownOS/crownos-android/spikes/run-wave0.sh s2 s1      # only some phases
#
# It starts a nested crownpositor window and points Waydroid at it, so Android
# never opens windows on your desktop. You type your sudo password once. A few
# steps ask you to confirm what you see (y/N).
#
# When it ends, for any reason, it stops Android, restores the stock
# hwcomposer (S2), removes the S1 overlay files, and closes the nested window.
# Everything it prints is also saved in ~/.cache/crownos-android/wave0/<time>/.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
compositor=/e/CrownOS/crownpositor/target/release/compositor
results=$HOME/.cache/crownos-android/wave0/$(date +%Y%m%d-%H%M%S)
phases=("$@")
((${#phases[@]})) || phases=(s3 s2 s1)

die() { echo "run-wave0: $*" >&2; exit 1; }
step() { printf '\n==== %s ====\n' "$*"; }

[[ $EUID -ne 0 ]] || die "run as your desktop user, not root; it asks for sudo itself"
[[ -n ${WAYLAND_DISPLAY:-} && -n ${XDG_RUNTIME_DIR:-} ]] || die "run inside your Wayland desktop session"
[[ -x $compositor ]] || die "missing $compositor; build crownpositor first"
waydroid status 2>/dev/null | grep -q "Session:.*STOPPED" || die "stop Waydroid first: waydroid session stop"

mkdir -p "$results"
exec > >(tee -a "$results/runbook.log") 2>&1
echo "results: $results"

step "sudo (asked once, kept alive while the runbook runs)"
sudo -v
while sleep 50; do sudo -n true 2>/dev/null || exit; done &
sudo_keepalive=$!

nested_pid=''
failures=()
config_dir=$(mktemp -d)

stop_android() {
    waydroid session stop >/dev/null 2>&1 || true
    sudo waydroid container stop >/dev/null 2>&1 || true
}

cleanup() {
    local status=$?
    step "restoring stock Waydroid"
    stop_android
    sudo "$here/s2-hwc-build/root-steps.sh" restore || echo "WARNING: S2 restore failed; run it again by hand"
    sudo "$here/s1-windowing/root-steps.sh" uninstall || echo "WARNING: S1 uninstall failed; run it again by hand"
    [[ -n $nested_pid ]] && kill "$nested_pid" 2>/dev/null || true
    kill "$sudo_keepalive" 2>/dev/null || true
    rm -rf "$config_dir"
    ((${#failures[@]})) && printf 'phases with failed checks: %s\n' "${failures[*]}"
    echo "done (exit $status). results: $results"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

wayland_sockets() {
    find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-[0-9]*' -printf '%f\n' | sort
}

step "starting a nested crownpositor"
before=$(wayland_sockets)
# An empty config dir gives the nested compositor the defaults: no startup
# apps, so no second wallpaper, bar or polkit agent. A nested compositor never
# exports its environment to D-Bus or systemd (compositor/src/lib.rs).
CROWN_CONFIG_DIR=$config_dir RUST_LOG=info "$compositor" >"$results/nested-compositor.log" 2>&1 &
nested_pid=$!
nested=''
for _ in $(seq 1 50); do
    nested=$(comm -13 <(printf '%s\n' "$before") <(wayland_sockets) | head -1)
    [[ -n $nested ]] && break
    kill -0 "$nested_pid" 2>/dev/null || die "nested compositor exited; see $results/nested-compositor.log"
    sleep 0.2
done
[[ -n $nested ]] || die "nested compositor did not create a socket within 10 s"
export WAYLAND_DISPLAY=$nested
echo "Waydroid will use the nested compositor on $WAYLAND_DISPLAY"

# A test that reports failed checks is a result, not a reason to skip the rest.
run_test() {
    local label=$1
    shift
    if ! "$@"; then
        failures+=("$label")
        echo "run-wave0: $label reported failures; continuing"
    fi
}

run_s3() {
    step "S3 platform baseline (stock Waydroid, no overlays)"
    run_test "S3 baseline" env S3_RESULTS="$results/s3" "$here/s3-platform/measure/run-baseline.sh"
    stop_android
}

run_s2() {
    step "S2 hwcomposer: stock module"
    sudo "$here/s2-hwc-build/root-steps.sh" restore
    run_test "S2 stock" "$here/s2-hwc-build/runtime-test.sh" stock
    stop_android
    step "S2 hwcomposer: CrownOS-built module"
    sudo "$here/s2-hwc-build/root-steps.sh" install
    run_test "S2 built" "$here/s2-hwc-build/runtime-test.sh" built
    stop_android
    sudo "$here/s2-hwc-build/root-steps.sh" restore
}

run_s1() {
    step "S1 Android windowing"
    "$here/s1-windowing/agent/build.sh"
    "$here/s1-windowing/overlays/build.sh"
    sudo "$here/s1-windowing/root-steps.sh" install
    run_test "S1 windowing" "$here/s1-windowing/runtime-test.sh"
    stop_android
    sudo "$here/s1-windowing/root-steps.sh" uninstall
}

for phase in "${phases[@]}"; do
    case $phase in
        s3) run_s3 ;;
        s2) run_s2 ;;
        s1) run_s1 ;;
        *) die "unknown phase '$phase' (s3, s2, s1)" ;;
    esac
done
