#!/usr/bin/env bash
# S2 / H4 runtime test: does the out-of-tree hwcomposer.waydroid.so load and render?
# Run it as your desktop user (it calls sudo for 'waydroid shell' and 'logcat'):
#
#   sudo ./root-steps.sh restore && ./runtime-test.sh stock    # baseline
#   sudo ./root-steps.sh install && ./runtime-test.sh built    # candidate
#
# Each run stops any running session, starts a fresh one, opens the full UI and
# saves logcat and dumpsys output under results/<label>-<time>/.
#
# Expected results, identical for 'stock' and 'built':
#   R1 overlay       built: overlay file == the built .so; stock: no overlay file
#   R2 boot          sys.boot_completed=1 within 240 s
#   R3 module        sha256 of /vendor/lib64/hw/hwcomposer.waydroid.so inside the
#                    container == the overlay file (built) or vendor.img's copy (stock)
#   R4 loaded        the composer service maps /vendor/lib64/hw/hwcomposer.waydroid.so
#   R5 link errors   no "CANNOT LINK", "cannot locate symbol", "dlopen failed",
#                    "library ... not found" or hw_get_module failure in logcat
#   R6 stable        same composer pid 15 s apart, no "Fatal signal" from it
#   R7 display       SurfaceFlinger reports at least one display
#   R8 window        a toplevel with app_id "Waydroid" exists after show-full-ui
#                    (checked with lswt or hyprctl, otherwise you are asked)
#   R9 renders       you confirm the window shows the Android home screen and
#                    reacts to a click
# Any FAIL means H4 is NO-GO for that build.
set -uo pipefail

LABEL=${1:-built}
BUILT=${2:-$HOME/.cache/crownos-android/s2/out/x86_64/vendor/lib64/hw/hwcomposer.waydroid.so}
MODULE=/vendor/lib64/hw/hwcomposer.waydroid.so
OVERLAY_FILE=/var/lib/waydroid/overlay$MODULE
STOCK_BACKUP=/var/lib/waydroid/crownos-backup/s2/hwcomposer.waydroid.so.stock
COMPOSER=android.hardware.graphics.composer@2.1-service
BOOT_TIMEOUT=240
RESULTS=$(cd "$(dirname "$0")" && pwd)/results/$LABEL-$(date +%Y%m%dT%H%M%S)
LINK_ERRORS='CANNOT LINK|cannot locate symbol|dlopen failed|library "[^"]+" not found|hw_get_module.*(fail|error)|Failed to load.*hwcomposer|failed to open wayland connection'

failures=0
mkdir -p "$RESULTS"

report() {
    local id=$1 verdict=$2 detail=$3
    printf '%-3s %-4s %s\n' "$id" "$verdict" "$detail" | tee -a "$RESULTS/summary.txt"
    [[ $verdict == PASS ]] || failures=$((failures + 1))
}

android_shell() { sudo waydroid shell -- "$@" 2> /dev/null; }
composer_pid() { android_shell pidof "$COMPOSER" | tr -d '\r'; }

expected_module_hash() {
    if [[ $LABEL == stock ]]; then
        sudo sha256sum "$STOCK_BACKUP" 2> /dev/null | cut -d' ' -f1
    else
        sha256sum "$BUILT" | cut -d' ' -f1
    fi
}

check_overlay() {
    if [[ $LABEL == stock ]]; then
        [[ ! -e $OVERLAY_FILE ]] && report R1 PASS "no overlay file" || report R1 FAIL "$OVERLAY_FILE exists"
    elif cmp -s "$OVERLAY_FILE" "$BUILT"; then
        report R1 PASS "overlay file matches $BUILT"
    else
        report R1 FAIL "overlay file missing or different from $BUILT (run root-steps.sh install)"
    fi
}

start_session() {
    waydroid session stop > /dev/null 2>&1 || true
    sudo waydroid container stop > /dev/null 2>&1 || true
    nohup waydroid session start > "$RESULTS/session.log" 2>&1 &
    local waited=0
    until [[ $(android_shell getprop sys.boot_completed | tr -d '\r') == 1 ]]; do
        if ((waited >= BOOT_TIMEOUT)); then
            report R2 FAIL "boot not completed after ${BOOT_TIMEOUT}s"
            return 1
        fi
        sleep 5
        waited=$((waited + 5))
    done
    report R2 PASS "boot completed after ~${waited}s"
}

check_module() {
    local inside expected
    inside=$(android_shell sha256sum "$MODULE" | cut -d' ' -f1)
    expected=$(expected_module_hash)
    [[ -n $inside && $inside == "$expected" ]] && report R3 PASS "container module sha256 $inside" ||
        report R3 FAIL "container module sha256 '$inside', expected '$expected'"
}

check_loaded() {
    local pid
    pid=$(composer_pid)
    if [[ -n $pid ]] && android_shell grep -q "$MODULE" "/proc/$pid/maps"; then
        report R4 PASS "$COMPOSER (pid $pid) maps $MODULE"
    else
        report R4 FAIL "$COMPOSER pid '$pid' does not map $MODULE"
    fi
}

check_logcat() {
    sudo waydroid logcat -- -d > "$RESULTS/logcat.txt" 2>&1
    if grep -Eq "$LINK_ERRORS" "$RESULTS/logcat.txt"; then
        report R5 FAIL "$(grep -Ec "$LINK_ERRORS" "$RESULTS/logcat.txt") link/load errors, see logcat.txt"
    else
        report R5 PASS "no link or HAL load errors"
    fi
}

check_stable() {
    local before after
    before=$(composer_pid)
    sleep 15
    after=$(composer_pid)
    sudo waydroid logcat -- -d > "$RESULTS/logcat-after.txt" 2>&1
    if [[ -n $before && $before == "$after" ]] && ! grep -q "Fatal signal.*$COMPOSER" "$RESULTS/logcat-after.txt"; then
        report R6 PASS "composer pid $before stable"
    else
        report R6 FAIL "composer pid '$before' -> '$after' or fatal signal logged"
    fi
}

check_display() {
    android_shell dumpsys SurfaceFlinger > "$RESULTS/dumpsys-surfaceflinger.txt"
    local displays
    displays=$(android_shell dumpsys SurfaceFlinger --display-id | grep -c 'Display')
    ((displays > 0)) && report R7 PASS "$displays display(s)" || report R7 FAIL "no display reported"
}

waydroid_window_listed() {
    if command -v lswt > /dev/null; then
        lswt 2> /dev/null | grep -q 'Waydroid'
    elif [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]] && command -v hyprctl > /dev/null; then
        hyprctl clients 2> /dev/null | grep -q 'class: Waydroid'
    else
        return 2
    fi
}

ask() {
    local answer
    read -r -t 120 -p "$1 [y/N] " answer || true
    [[ $answer == [yY]* ]]
}

check_window() {
    waydroid show-full-ui > "$RESULTS/show-full-ui.log" 2>&1 &
    sleep 10
    waydroid_window_listed
    case $? in
        0) report R8 PASS "toplevel 'Waydroid' listed by the compositor" ;;
        1) report R8 FAIL "no toplevel 'Waydroid' listed by the compositor" ;;
        *) ask "R8: is a window titled 'Waydroid' open?" && report R8 PASS "confirmed by user" ||
            report R8 FAIL "user did not confirm the window" ;;
    esac
    ask "R9: does it show the Android home screen and react to a click?" &&
        report R9 PASS "confirmed by user" || report R9 FAIL "user did not confirm rendering/input"
}

echo "S2 runtime test '$LABEL' -> $RESULTS"
check_overlay
if start_session; then
    check_module
    check_loaded
    check_logcat
    check_stable
    check_display
    check_window
fi
echo "$failures failure(s)" | tee -a "$RESULTS/summary.txt"
exit $((failures > 0))
