#!/usr/bin/env bash
# S1 runtime experiments. Needs your approval: it starts, restarts and stops the Waydroid session.
#
# Before running:
#   agent/build.sh && overlays/build.sh      (as your user)
#   sudo ./root-steps.sh install             (overlay files + adb key)
#
# Usage: ./runtime-test.sh [phase ...]
#   phases: boot launch latency slots shadows multiwindow ime-toast captions home   (default: all, in that order)
#
# Every step prints its expected result next to it. Results go to
# ~/.cache/crownos-android/s1/runtime/<timestamp>/results.tsv (step, verdict, expected, observed);
# REVIEW means the observed value needs a human look (screenshots are saved next to it).
# Steps keep going after a failed check; only setup failures (boot, adb, push) stop the run.
set -uo pipefail

spike_dir="$(cd "$(dirname "$0")" && pwd)"
run_dir="$HOME/.cache/crownos-android/s1/runtime/$(date +%Y%m%d-%H%M%S)"
results="$run_dir/results.tsv"
device_jar=/data/local/tmp/crown-agent-spike.jar
root_fifo=/data/local/tmp/crown-agent-spike.in
root_log=/data/local/tmp/crown-agent-spike.out
boot_timeout_s=240
adb_target=""

calculator=com.android.calculator2
clock=com.android.deskclock
browser=org.lineageos.jelly
search_activity=com.android.settings.intelligence/com.android.settings.intelligence.search.SearchActivity
home_component=org.crownos.spike.home/android.app.Activity

mkdir -p "$run_dir"
printf 'step\tverdict\texpected\tobserved\n' > "$results"

log() { printf '\n== %s\n' "$*"; }

record() {
    local step="$1" verdict="$2" expected="$3" observed="$4"
    observed="$(printf '%s' "$observed" | tr '\n\t' '  ' | cut -c1-400)"
    printf '%s\t%s\t%s\t%s\n' "$step" "$verdict" "$expected" "$observed" >> "$results"
    printf '  [%s] %s\n      expected: %s\n      observed: %s\n' "$verdict" "$step" "$expected" "$observed"
}

flatten() {
    printf '%s' "$1" | tr '\r\n' '  ' | sed 's/[[:space:]]*$//'
}

expect_match() {
    local step="$1" expected="$2" pattern="$3" observed="$4"
    if grep -Eq -- "$pattern" <<< "$(flatten "$observed")"; then
        record "$step" PASS "$expected" "$observed"
    else
        record "$step" FAIL "$expected" "$observed"
    fi
}

expect_no_match() {
    local step="$1" expected="$2" pattern="$3" observed="$4"
    if grep -Eq -- "$pattern" <<< "$(flatten "$observed")"; then
        record "$step" FAIL "$expected" "$observed"
    else
        record "$step" PASS "$expected" "$observed"
    fi
}

adb_device() { adb -s "$adb_target" "$@"; }
android_shell() { adb_device shell "$@"; }

# Runs the agent as uid 2000 (shell) through adb.
agent_shell() {
    android_shell "CLASSPATH=$device_jar app_process / org.crownos.agent.Spike $*" || true
}

# Runs the agent as uid 0 through the init service started from the overlay (serve mode).
agent_root() {
    local wait_s="${AGENT_ROOT_WAIT_S:-3}"
    local before
    before="$(android_shell "wc -l < $root_log" | tr -d '\r ')"
    android_shell "echo '$*' > $root_fifo"
    sleep "$wait_s"
    android_shell "tail -n +$((before + 1)) $root_log"
}

display_size() {
    android_shell wm size | tr -d '\r' | awk -F': ' '/Override size/ {o=$2} /Physical size/ {p=$2} END {print (o != "" ? o : p)}'
}

task_of() {
    grep -oE 'task=[0-9]+' <<< "$1" | head -1 | cut -d= -f2 || true
}

focused_app() {
    android_shell 'dumpsys window' | tr -d '\r' | grep -m1 mFocusedApp || true
}

resumed_packages() {
    android_shell 'dumpsys activity activities' | tr -d '\r' | awk '
        /\* Hist #|ActivityRecord\{/ { if (match($0, /u0 [^ \/]+/)) pkg = substr($0, RSTART + 3, RLENGTH - 3) }
        /state=RESUMED/ && pkg != "" { print pkg }' | sort -u | tr '\n' ' ' | sed 's/ $//'
}

screenshot() {
    android_shell screencap -p "/data/local/tmp/s1-$1.png"
    adb_device pull "/data/local/tmp/s1-$1.png" "$run_dir/$1.png" > /dev/null
    echo "      screenshot: $run_dir/$1.png"
}

session_is_running() {
    waydroid status 2>/dev/null | grep -Eq 'Session:[[:space:]]+RUNNING'
}

container_ip() {
    grep -oE '([0-9]{1,3}\.){3}[0-9]{1,3}' /var/lib/misc/dnsmasq.waydroid0.leases 2>/dev/null | tail -1 || true
}

wait_for_boot() {
    local deadline=$((SECONDS + boot_timeout_s))
    until session_is_running; do
        (( SECONDS < deadline )) || { echo "session did not start" >&2; exit 1; }
        sleep 2
    done
    local ip=""
    until [[ -n "$ip" ]]; do
        ip="$(container_ip)"
        (( SECONDS < deadline )) || { echo "no container IP" >&2; exit 1; }
        [[ -n "$ip" ]] || sleep 2
    done
    adb_target="$ip:5555"
    until adb connect "$adb_target" | grep -q connected && \
          [[ "$(android_shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; do
        (( SECONDS < deadline )) || { echo "boot not completed or adb unauthorized (see root-steps.sh step 10)" >&2; exit 1; }
        sleep 3
    done
    android_shell "input keyevent KEYCODE_WAKEUP" || true
    adb_device push "$spike_dir/agent/build/crown-agent-spike.jar" "$device_jar" > /dev/null || {
        echo "cannot push the agent jar; run agent/build.sh" >&2
        exit 1
    }
}

session_start() {
    session_is_running || { waydroid session start > "$run_dir/session.log" 2>&1 & }
    wait_for_boot
}

session_restart() {
    if session_is_running; then
        [[ -z "$adb_target" ]] || adb disconnect "$adb_target" > /dev/null 2>&1 || true
        waydroid session stop || true
        sleep 3
    fi
    session_start
}

# Applies Android properties that are read once at zygote/SystemUI start, then reboots Android.
configure_session() {
    local caption_on_shell="$1"
    session_start
    waydroid prop set persist.waydroid.multi_windows true
    waydroid prop set persist.wm.debug.caption_on_shell "$caption_on_shell"
    session_restart
    record "session.props caption_on_shell=$caption_on_shell" REVIEW \
        "multi_windows=true caption_on_shell=$caption_on_shell" \
        "$(android_shell 'getprop persist.waydroid.multi_windows; getprop persist.wm.debug.caption_on_shell')"
}

slot_bounds() {
    local index="$1" size width height half_w half_h gap=24
    size="$(display_size)"
    width="${size%x*}"
    height="${size#*x}"
    half_w=$(( (width - 3 * gap) / 2 ))
    half_h=$(( (height - 3 * gap) / 2 ))
    case "$index" in
        0) echo "$gap $gap $((gap + half_w)) $((gap + half_h))" ;;
        1) echo "$((2 * gap + half_w)) $gap $((2 * gap + 2 * half_w)) $((gap + half_h))" ;;
        2) echo "$gap $((2 * gap + half_h)) $((gap + half_w)) $((2 * gap + 2 * half_h))" ;;
        *) echo "$((2 * gap + half_w)) $((2 * gap + half_h)) $((2 * gap + 2 * half_w)) $((2 * gap + 2 * half_h))" ;;
    esac
}

launch_in_slot() {
    local target="$1" slot="$2" mode="${3:-freeform}"
    android_shell "am force-stop ${target%%/*}"
    agent_shell launch "$target" $(slot_bounds "$slot") "$mode"
}

phase_boot() {
    log "boot: shell access and the init.rc root agent (G5)"
    configure_session false

    expect_match "shell.uid" "adb shell runs as uid=2000(shell); ro.adb.secure=1" \
        'uid=2000\(shell\).*1$' "$(android_shell 'id; getprop ro.adb.secure' | tr -d '\r' | tr '\n' ' ')"
    expect_match "display.freeform" "pc.xml bound by init.waydroid.rc, so the display windowing mode is freeform" \
        'android.hardware.type.pc' "$(android_shell 'pm list features' | grep 'type.pc' || true)"
    expect_match "overlay.files" "overlay files visible at /system/etc/init and /system/framework" \
        'crown-agent-spike.rc.*crown-agent-spike.jar|crown-agent-spike.jar.*crown-agent-spike.rc' \
        "$(android_shell 'ls /system/etc/init/crown-agent-spike.rc /system/framework/crown-agent-spike.jar' | tr '\n' ' ')"
    expect_match "init.service" "init.svc.crown_agent_spike=running and ro.boottime.crown_agent_spike set" \
        '^running [0-9]+' "$(android_shell 'getprop init.svc.crown_agent_spike; getprop ro.boottime.crown_agent_spike' | tr -d '\r' | tr '\n' ' ')"
    expect_match "init.service.uid" "serve.ready line reports uid=0" \
        'serve.ready uid=0' "$(android_shell "grep serve.ready $root_log | tail -1")"
    expect_match "root.list-tasks" "root agent lists tasks (uid 0 passes MANAGE_ACTIVITY_TASKS / REAL_GET_TASKS)" \
        'tasks.count count=[1-9]' "$(agent_root list-tasks)"
    expect_match "shell.list-tasks" "shell agent lists tasks (shell holds MANAGE_ACTIVITY_TASKS)" \
        'tasks.count count=[1-9]' "$(agent_shell list-tasks)"
    android_shell "settings put global policy_control 'immersive.full=*'"
    sleep 2
    record "policy_control" REVIEW "patch 0029: immersive.full=* gives StatusBar and NavigationBar windows zero height" \
        "$(android_shell 'dumpsys window windows' | tr -d '\r' | grep -A40 -E 'Window\{.*(StatusBar|NavigationBar)\}' | grep -E 'Window\{|frame=' | head -4)"
}

phase_launch() {
    log "launch, resize, focus, info (G1)"
    agent_shell watch 60 > "$run_dir/watch-launch.log" 2>&1 &
    local watcher=$! launch task
    sleep 2

    launch="$(launch_in_slot "$calculator" 0)"
    task="$(task_of "$(grep launch.task <<< "$launch")")"
    expect_match "launch.freeform" "launch.task line with mode=freeform and bounds equal to slot 0 ($(slot_bounds 0 | tr ' ' ','))" \
        "mode=freeform bounds=$(slot_bounds 0 | tr ' ' ',')" "$launch"

    expect_match "resize.wct.shell" "WCT setBounds from uid 2000 applies: bounds=100,100,900,700" \
        'bounds=100,100,900,700' "$(agent_shell resize-wct "$task" 100 100 900 700; sleep 1; agent_shell info "$task")"
    expect_match "resize.legacy.shell" "resizeTask from uid 2000 returns true and applies: bounds=120,120,1020,820" \
        'result=true.*bounds=120,120,1020,820' "$(agent_shell resize-legacy "$task" 120 120 1020 820; sleep 1; agent_shell info "$task")"
    expect_match "resize.wct.root" "WCT setBounds from uid 0 applies: bounds=$(slot_bounds 0 | tr ' ' ',')" \
        "bounds=$(slot_bounds 0 | tr ' ' ',')" "$(agent_root resize-wct "$task" $(slot_bounds 0); agent_shell info "$task")"

    local second
    second="$(task_of "$(launch_in_slot "$clock" 1 | grep launch.task)")"
    agent_shell focus "$task" > /dev/null
    sleep 1
    expect_match "focus" "after focus $task: mFocusedApp belongs to $calculator" "$calculator" "$(focused_app)"
    agent_shell front "$second" > /dev/null
    sleep 1
    expect_match "front" "after front $second: mFocusedApp belongs to $clock" "$clock" "$(focused_app)"

    record "info" REVIEW "label=Calculator, app_label=Calculator, colors set, min sizes and resizeMode reported" \
        "$(agent_shell info "$task")"
    expect_match "remove" "removeTask returns true and the task disappears" \
        'result=true' "$(agent_shell remove "$second")"

    wait "$watcher" || true
    expect_match "watch.events" "task stack events with t=<CLOCK_MONOTONIC ns> for created, focus, front, removed" \
        'stack.created.*task=|stack.removed' "$(cat "$run_dir/watch-launch.log")"
}

phase_latency() {
    log "WCT resize latency (G3): BLAST sync callback and SurfaceFlinger frame timing"
    local task line latencies=() size=0
    task="$(task_of "$(launch_in_slot "$calculator" 0 | grep launch.task)")"
    sleep 2
    for run in $(seq 1 12); do
        if (( run % 2 )); then size=700; else size=900; fi
        line="$(agent_shell resize-wct "$task" 100 100 $((100 + size)) $((100 + size * 3 / 4)) --sync | grep resize.wct.sync || true)"
        echo "$line" >> "$run_dir/latency.log"
        latencies+=("$(grep -oE 'latency_ms=[0-9.]+' <<< "$line" | cut -d= -f2 || true)")
        sleep 0.5
    done
    local stats
    stats="$(printf '%s\n' "${latencies[@]}" | grep -E '^[0-9.]+$' | sort -n | \
        awk '{v[NR]=$1} END {if (NR == 0) {print "none"; exit} print "n=" NR " median=" v[int((NR+1)/2)] " p95=" v[int(NR*0.95+0.999)] " max=" v[NR]}')"
    local p95
    p95="$(grep -oE 'p95=[0-9.]+' <<< "$stats" | cut -d= -f2 || true)"
    if [[ -n "$p95" ]] && awk "BEGIN {exit !($p95 <= 250)}"; then
        record "latency.wct.sync" PASS "p95 of command -> onTransactionReady (app redrawn at new size) <= 250 ms" "$stats"
    else
        record "latency.wct.sync" FAIL "p95 of command -> onTransactionReady (app redrawn at new size) <= 250 ms" "$stats"
    fi

    local t_cmd layer frames
    t_cmd="$(agent_shell resize-wct "$task" 100 100 900 700 | grep -oE 't_cmd=[0-9]+' | cut -d= -f2)"
    sleep 1
    layer="$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r' | grep "TID:$task#" | grep -v -E 'Background|Decor|Caption|Snapshot|Splash' | head -1)"
    frames="$(android_shell "dumpsys SurfaceFlinger --latency '$layer'" | tr -d '\r' | \
        awk -v t="$t_cmd" 'NF == 3 && $1 > t && $1 < 9e18 {printf "first_frame_after_cmd_ms=%.1f ready_ms=%.1f\n", ($1 - t) / 1e6, ($3 - t) / 1e6; exit}')"
    record "latency.wct.sf" REVIEW "first frame of layer '$layer' after a non-sync WCT resize, <= 250 ms" "${frames:-no frame found}"
    record "latency.wct.size" REVIEW "SurfaceFlinger buffer for that layer is 800x600 after the resize" \
        "$(android_shell 'dumpsys SurfaceFlinger' | tr -d '\r' | grep -A25 -F "$layer" | grep -m2 -E 'activeBuffer|bufferSize|size=')"
}

phase_slots() {
    log "non-overlapping slots stay visible and resumed (G4)"
    android_shell "am force-stop $calculator; am force-stop $clock; am force-stop $browser"
    launch_in_slot "$calculator" 0 > /dev/null
    launch_in_slot "$clock" 1 > /dev/null
    launch_in_slot "$browser" 2 > /dev/null
    sleep 10
    local expected_resumed
    expected_resumed="$(printf '%s\n' "$browser" "$calculator" "$clock" | sort | tr '\n' ' ' | sed 's/ $//')"
    expect_match "slots.resumed" "all three apps RESUMED 10 s after launch (multi-resume)" \
        "$expected_resumed" "$(resumed_packages)"
    expect_no_match "slots.visible" "list-tasks reports visible=true for all three tasks" \
        'visible=false.*(calculator2|deskclock|jelly)' "$(agent_shell list-tasks | grep -E "$calculator|$clock|$browser")"
    for slot in 0 1 2; do
        agent_shell focus "$(agent_shell list-tasks | grep -E "base=[^ ]*($calculator|$clock|$browser)" | sed -n "$((slot + 1))p" | grep -oE 'task=[0-9]+' | cut -d= -f2)" > /dev/null
        sleep 1
    done
    expect_match "slots.resumed.after-focus" "still all three RESUMED after moving focus between them" \
        "$expected_resumed" "$(resumed_packages)"
    screenshot slots
}

phase_shadows() {
    log "client-side shadows (patch 0028) and how to turn them off"
    android_shell "settings delete global render_shadows_in_compositor"
    launch_in_slot "$calculator" 0 > /dev/null
    sleep 2
    expect_match "shadows.client.default" "freeform app window has non-zero surfaceInsets (5 dp DecorView elevation)" \
        'surfaceInsets=' "$(android_shell 'dumpsys window windows' | grep -A12 "$calculator" | grep -m1 -oE 'surfaceInsets=Rect\([^)]*\)' || true)"
    android_shell "settings put global render_shadows_in_compositor 1"
    launch_in_slot "$calculator" 0 > /dev/null
    sleep 2
    expect_no_match "shadows.client.off" "after render_shadows_in_compositor=1 and an app restart: no surfaceInsets" \
        'surfaceInsets=' "$(android_shell 'dumpsys window windows' | grep -A12 "$calculator" | grep -m1 -oE 'surfaceInsets=Rect\([^)]*\)' || true)"
    record "shadows.layers" REVIEW "shadowRadius only on WM Shell 'TID:<id>#pkg/Background' layers (removed by the decor-free options)" \
        "$(android_shell 'dumpsys SurfaceFlinger' | tr -d '\r' | grep -E '^\+ |shadowRadius=[1-9]' | grep -B1 -E 'shadowRadius=[1-9]' | head -6)"
}

phase_multiwindow() {
    log "MULTI_WINDOW root tasks: visibility, caption, resize, input, SystemUI crash"
    local launch task
    launch="$(launch_in_slot "$clock" 3 multiwindow)"
    task="$(task_of "$(grep launch.task <<< "$launch")")"
    expect_match "multiwindow.mode" "task launched with mode=multiwindow visible=true" \
        'mode=multiwindow.*visible=true' "$launch"
    expect_match "multiwindow.resumed" "$clock RESUMED" "$clock" "$(resumed_packages)"
    record "multiwindow.legacy-caption" REVIEW \
        "DecorCaptionView present when caption_on_shell=false (display mode is freeform, see hasWindowDecorCaption)" \
        "DecorCaptionView count: $(android_shell "dumpsys activity activity $clock" | grep -c DecorCaptionView || true)"
    expect_no_match "multiwindow.no-shell-decor" "no WM Shell decor layers (no shell listener for MULTI_WINDOW)" \
        "TID:$task#.*(CaptionContainer|Background|DecorContainer)" "$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r')"
    local before_pos after_pos
    before_pos="$(android_shell 'dumpsys SurfaceFlinger' | tr -d '\r' | grep -A8 -E "\(Task=$task#" | grep -m1 -oE 'pos=\([^)]*\)' || true)"
    agent_shell resize-wct "$task" 200 200 1000 800 > /dev/null
    sleep 1
    after_pos="$(android_shell 'dumpsys SurfaceFlinger' | tr -d '\r' | grep -A8 -E "\(Task=$task#" | grep -m1 -oE 'pos=\([^)]*\)' || true)"
    record "multiwindow.leash-position" REVIEW \
        "without our organizer nobody moves the task leash after a bounds change (pos stays as before)" \
        "before=$before_pos after=$after_pos bounds=$(agent_shell info "$task" | grep -oE 'bounds=[0-9,]+')"
    android_shell "input tap 400 400"
    sleep 1
    record "multiwindow.input" REVIEW "a tap inside the new bounds focuses $clock" \
        "$(android_shell 'dumpsys window' | tr -d '\r' | grep -m1 mCurrentFocus)"
    android_shell "am crash com.android.systemui" || true
    sleep 8
    expect_match "multiwindow.after-systemui-crash" "task still listed, visible and multiwindow after SystemUI restarts" \
        "task=$task mode=multiwindow.*visible=true" "$(agent_shell list-tasks)"
}

phase_ime_toast() {
    log "IME and toast placement"
    local launch task
    launch="$(launch_in_slot "$search_activity" 0)"
    task="$(task_of "$(grep launch.task <<< "$launch")")"
    sleep 3
    record "ime.shown" REVIEW "mInputShown=true for the search field" \
        "$(android_shell 'dumpsys input_method' | tr -d '\r' | grep -m2 -E 'mInputShown|mCurFocusedWindow=')"
    record "ime.frame" REVIEW "InputMethod window frame sits at the bottom of the display, outside slot 0 ($(slot_bounds 0 | tr ' ' ','))" \
        "$(android_shell 'dumpsys window windows' | tr -d '\r' | grep -A40 'Window{.*InputMethod}' | grep -m2 -E 'frame=|mFrame=')"
    record "ime.layer" REVIEW "SurfaceFlinger layer named InputMethod without a TID: prefix" \
        "$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r' | grep -i inputmethod | head -3)"
    screenshot ime
    agent_shell toast "S1 toast placement" > "$run_dir/toast.log" 2>&1 &
    sleep 1.5
    record "toast.window" REVIEW "TYPE_TOAST (2005) window owned by SystemUI, bottom-centre of the display, not in task $task" \
        "$(android_shell 'dumpsys window windows' | tr -d '\r' | grep -A40 -E 'Window\{.*Toast' | grep -m3 -E 'Window\{|ty=|frame=|mFrame=')"
    record "toast.layer" REVIEW "toast layer has no TID: prefix, so the stock multi-window HWC drops it" \
        "$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r' | grep -i toast | head -3)"
    screenshot toast
    wait || true
}

caption_report() {
    local label="$1" task="$2"
    record "caption.$label.legacy" REVIEW "DecorCaptionView count in $calculator (legacy app-side caption)" \
        "$(android_shell "dumpsys activity activity $calculator" | grep -c DecorCaptionView || true)"
    record "caption.$label.shell-layers" REVIEW "WM Shell decor layers for task $task" \
        "$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r' | grep "TID:$task#" | grep -E 'Caption|Background|Decor' | tr '\n' ' ')"
    record "caption.$label.insets" REVIEW "caption insets sources (ITYPE_CAPTION_BAR) known to WindowManager" \
        "$(android_shell 'dumpsys window' | tr -d '\r' | grep -i -m3 'caption_bar\|CAPTION_BAR' || true)"
    screenshot "caption-$label"
}

phase_captions() {
    log "caption options (G2)"
    local task

    configure_session false
    task="$(task_of "$(launch_in_slot "$calculator" 0 | grep launch.task)")"
    sleep 2
    caption_report legacy "$task"
    expect_match "caption.legacy.present" "stock build: legacy DecorCaptionView drawn in the app (caption_on_shell=false)" \
        '^[1-9]' "$(android_shell "dumpsys activity activity $calculator" | grep -c DecorCaptionView || true)"

    configure_session true
    task="$(task_of "$(launch_in_slot "$calculator" 0 | grep launch.task)")"
    sleep 2
    caption_report shell "$task"
    expect_match "caption.shell.legacy-gone" "caption_on_shell=true removes the legacy DecorCaptionView" \
        '^0' "$(android_shell "dumpsys activity activity $calculator" | grep -c DecorCaptionView || true)"

    expect_match "caption.rro.enabled" "SystemUI RRO enabled: [x] org.crownos.spike.overlay.systemui" \
        '\[x\].*org.crownos.spike.overlay.systemui' \
        "$(android_shell 'cmd overlay enable org.crownos.spike.overlay.systemui; cmd overlay list com.android.systemui' | tr -d '\r')"
    android_shell "am crash com.android.systemui" || true
    sleep 8
    task="$(task_of "$(launch_in_slot "$calculator" 0 | grep launch.task)")"
    sleep 2
    caption_report rro "$task"
    record "caption.rro.result" REVIEW \
        "with the RRO: caption height 0, no caption insets, shadowRadius 0, no resize handles; app content starts at the task top" \
        "see caption-rro.png and the rows above"
    android_shell "cmd overlay disable org.crownos.spike.overlay.systemui" || true
    android_shell "am crash com.android.systemui" || true
    sleep 8

    AGENT_ROOT_WAIT_S=3 agent_root organize > "$run_dir/organize-start.log"
    task="$(task_of "$(launch_in_slot "$calculator" 0 | grep launch.task)")"
    sleep 2
    caption_report organizer "$task"
    expect_no_match "caption.organizer.no-decor" "agent is TaskOrganizer: no WM Shell decor layers for task $task" \
        "TID:$task#.*(CaptionContainer|Background|DecorContainer)" "$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r')"
    agent_shell resize-wct "$task" 300 200 1100 800 > /dev/null
    sleep 1
    record "caption.organizer.leash" REVIEW "organizer moved the leash: Task=$task layer pos=(300,200), crop 800x600" \
        "$(android_shell 'dumpsys SurfaceFlinger' | tr -d '\r' | grep -A8 -E "\(Task=$task#" | grep -oE 'pos=\([^)]*\)|crop=\[[^]]*\]' | head -2 | tr '\n' ' ')"
    android_shell "am crash com.android.systemui" || true
    sleep 8
    expect_match "caption.organizer.reclaim" "after SystemUI restarts it re-registers; agent sees vanished still_running=true then reclaims" \
        'organizer.reclaim' "$(android_shell "tail -n 200 $root_log")"
    expect_no_match "caption.organizer.after-crash" "no WM Shell decor layers after the reclaim" \
        "TID:$task#.*(CaptionContainer|Background|DecorContainer)" "$(android_shell 'dumpsys SurfaceFlinger --list' | tr -d '\r')"
    screenshot organizer-after-systemui-crash
}

phase_home() {
    log "wallpaper disabled and blank HOME (overlay files from root-steps.sh)"
    expect_match "wallpaper.overlay" "static framework RRO enabled" \
        '\[[xX]\] org.crownos.spike.overlay.framework' "$(android_shell 'cmd overlay list android' | tr -d '\r' | grep spike || true)"
    expect_match "wallpaper.service" "config_enableWallpaperService=false: no 'wallpaper' system service" \
        'not found' "$(android_shell 'service check wallpaper' | tr -d '\r')"
    expect_match "home.set" "set-home-activity accepted" \
        'Success' "$(android_shell "cmd package set-home-activity $home_component" | tr -d '\r')"
    expect_match "home.launcher-disabled" "Trebuchet disabled for user 0 (it also hosts Recents, which SystemUI would rebind)" \
        'disabled-user' "$(android_shell 'pm disable-user --user 0 com.android.launcher3' | tr -d '\r')"
    android_shell "am force-stop com.android.launcher3; am start -W -a android.intent.action.MAIN -c android.intent.category.HOME" > /dev/null
    sleep 2
    expect_match "home.resumed" "home task runs $home_component" \
        'org.crownos.spike.home' "$(android_shell 'dumpsys activity activities' | tr -d '\r' | grep -m3 -E 'type=home|realActivity=.*home')"
    record "home.frame" REVIEW "blank HOME window is floating and empty (frame of 0 or 1 px), so it holds no display-sized buffer" \
        "$(android_shell 'dumpsys window windows' | tr -d '\r' | grep -A40 'org.crownos.spike.home' | grep -m2 -E 'frame=|mFrame=')"
    expect_no_match "home.launcher-stopped" "launcher process not running" \
        '[0-9]' "$(android_shell 'pidof com.android.launcher3' | tr -d '\r' || true)"
    screenshot home
    echo "      undo: adb shell pm enable com.android.launcher3; adb shell cmd package set-home-activity com.android.launcher3/com.android.launcher3.uioverrides.QuickstepLauncher"
}

main() {
    local phases=("$@")
    (( ${#phases[@]} )) || phases=(boot launch latency slots shadows multiwindow ime-toast captions home)
    for phase in "${phases[@]}"; do
        if [[ -z "$adb_target" && "$phase" != boot && "$phase" != captions ]]; then
            configure_session false
        fi
        "phase_${phase//-/_}"
    done
    log "results: $results"
    column -t -s $'\t' "$results" | cut -c1-220
}

main "$@"
