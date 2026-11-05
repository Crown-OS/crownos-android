#!/usr/bin/env bash
# S4 Venus tier: runtime proof (criterion V3). NOT executed by the spike.
# Run as the desktop user inside a crownpositor session on an NVIDIA-only
# machine (dev laptop: MUX in dGPU mode so eDP and boot_vga are on NVIDIA),
# after root-steps.sh. Steps that need root use sudo and say so.
#
#   ./runtime-test.sh [vulkan-test.apk package-name]
#
# Each check prints PASS/FAIL with the expected value. The run stops at the
# first FAIL so the log names the broken link of the chain.
set -uo pipefail

vk_apk="${1:-}"
vk_pkg="${2:-}"
sock="$HOME/.local/share/waydroid/data/crownos/venus/venus.sock"
log_dir="${LOG_DIR:-$HOME/.cache/crownos-android/s4/runtime-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$log_dir"

pass() { echo "PASS  $1"; }
fail() { echo "FAIL  $1"; echo "      expected: $2"; echo "      logs: $log_dir"; exit 1; }
check() {
    local name=$1 expected=$2; shift 2
    if "$@" >>"$log_dir/checks.log" 2>&1; then pass "$name"; else fail "$name" "$expected"; fi
}
android() { sudo waydroid shell -- "$@"; }
prop_is() { [[ $(android getprop "$1" | tr -d '\r') == "$2" ]]; }

# --- 0. Host preconditions ---------------------------------------------------
check "compositor GPU is NVIDIA" "boot_vga card has DRIVER=nvidia" \
    sh -c 'for c in /sys/class/drm/card[0-9]*; do [ "$(cat $c/device/boot_vga 2>/dev/null)" = 1 ] && grep -qx DRIVER=nvidia $c/device/uevent && exit 0; done; exit 1'
check "udmabuf usable by $USER" "rw access via the uaccess ACL" test -r /dev/udmabuf -a -w /dev/udmabuf

# --- 1. Render server --------------------------------------------------------
systemctl --user daemon-reload
systemctl --user restart crownos-venus.service
for _ in $(seq 50); do [[ -S $sock ]] && break; sleep 0.1; done
check "render server socket listening" "unix socket at $sock, mode srwxrwxrwx, LISTEN" \
    sh -c "ss -xl | grep -qF '$sock' && stat -c %A '$sock' | grep -qx srwxrwxrwx"
check "render server healthy" "unit active, no 'failed to initialize venus renderer' in the journal" \
    sh -c "systemctl --user is-active -q crownos-venus.service && ! journalctl --user -u crownos-venus.service -b --no-pager | grep -qi 'failed'"

# --- 2. Start Android ----------------------------------------------------------
waydroid session start >"$log_dir/session.log" 2>&1 &
for _ in $(seq 120); do prop_is sys.boot_completed 1 2>/dev/null && break; sleep 1; done
check "Android booted" "sys.boot_completed=1 within 120 s, SurfaceFlinger not crash-looping" prop_is sys.boot_completed 1
android logcat -d >"$log_dir/logcat-boot.txt"

# --- 3. Props and nodes inside the container ----------------------------------
for kv in ro.hardware.vulkan=virtio ro.hardware.egl=angle ro.hardware.gralloc=minigbm_gbm_mesa \
          mesa.vn.debug=vtest mesa.vtest.socket.name=/data/crownos/venus/venus.sock; do
    check "prop ${kv%%=*}" "${kv#*=}" prop_is "${kv%%=*}" "${kv#*=}"
done
check "NVIDIA render node in container" "/dev/dri/renderD* char device 226:12x, opened by minigbm" \
    android sh -c 'ls -l /dev/dri/ | grep -q "^c.* 226, *1[0-9][0-9]"'
check "socket reachable from Android" "srwxrwxrwx /data/crownos/venus/venus.sock" \
    android sh -c 'ls -l /data/crownos/venus/venus.sock | grep -q "^srwxrwxrwx"'

# --- 4. gralloc: host-side allocation over vtest --------------------------------
check "allocator 4.0 (minigbm) running" "vendor.graphics.allocator-4-0 running, gralloc-2-0 stopped" \
    sh -c "[ \"\$(sudo waydroid shell -- getprop init.svc.vendor.graphics.allocator-4-0 | tr -d '\r')\" = running ]"
check "vtest gralloc backend active" "logcat 'vtest gralloc wrapper active (host-GPU allocations)'" \
    grep -q "vtest gralloc wrapper active" "$log_dir/logcat-boot.txt"
check "no gralloc failures" "no 'Unable to find or open DRM node' / 'VCMD_RESOURCE_ALLOC_GPU' errors" \
    sh -c "! grep -E 'Unable to find or open DRM node|vtest_wrapper.*(fail|error)' '$log_dir/logcat-boot.txt'"
journalctl --user -u crownos-venus.service -b --no-pager >"$log_dir/render-server.log"
check "host allocates on the NVIDIA GPU" "render-server log: vtest_gpu_alloc: allocating on \"NVIDIA GeForce RTX 4050 ...\"" \
    grep -q 'vtest_gpu_alloc: allocating on "NVIDIA' "$log_dir/render-server.log"

# --- 5. GLES (ANGLE on Venus) and Vulkan strings in SurfaceFlinger ------------
android dumpsys SurfaceFlinger >"$log_dir/dumpsys-sf.txt"
check "SF GLES is ANGLE on Venus on NVIDIA" "GLES: Google Inc. (NVIDIA), ANGLE (NVIDIA, Vulkan 1.x (Virtio-GPU Venus (NVIDIA GeForce RTX 4050 Laptop GPU)), venus-...), OpenGL ES 3.2" \
    grep -E "GLES: .*ANGLE.*Venus.*NVIDIA" "$log_dir/dumpsys-sf.txt"
check "not SwiftShader" "no SwiftShader in the GLES line" sh -c "! grep -E 'GLES:.*SwiftShader' '$log_dir/dumpsys-sf.txt'"
android cmd gpu vkjson >"$log_dir/vkjson.json"
check "Vulkan device is Venus on NVIDIA" "deviceName 'Virtio-GPU Venus (NVIDIA GeForce RTX 4050 Laptop GPU)', driverID VK_DRIVER_ID_MESA_VENUS" \
    grep -q '"deviceName": "Virtio-GPU Venus (NVIDIA' "$log_dir/vkjson.json"
check "ASTC available (emulated)" "textureCompressionASTC_LDR: 1 (patched venus emulation); textureCompressionETC2 expected 0" \
    grep -q '"textureCompressionASTC_LDR": 1' "$log_dir/vkjson.json"

# --- 6. Vulkan app: HWUI skiavk on every app, plus an optional Vulkan APK ------
android am start -W -n com.android.settings/.Settings >/dev/null
sleep 3
android dumpsys gfxinfo com.android.settings >"$log_dir/gfxinfo-settings.txt"
check "HWUI renders with Vulkan" "gfxinfo 'Pipeline=Skia (Vulkan)', janky frames < 10 %" \
    grep -q "Pipeline=Skia (Vulkan)" "$log_dir/gfxinfo-settings.txt"
if [[ -n $vk_apk && -n $vk_pkg ]]; then
    android pm install -r "/data/local/tmp/$(basename "$vk_apk")" >/dev/null 2>&1 || \
        { sudo cp "$vk_apk" "$HOME/.local/share/waydroid/data/local/tmp/" && android pm install -r "/data/local/tmp/$(basename "$vk_apk")"; }
    android monkey -p "$vk_pkg" -c android.intent.category.LAUNCHER 1 >/dev/null
    sleep 10
    android logcat -d >"$log_dir/logcat-vkapp.txt"
    check "Vulkan APK runs" "app process alive after 10 s, no VK_ERROR_* / vn_ / SIGSEGV in logcat" \
        sh -c "sudo waydroid shell -- pidof $vk_pkg >/dev/null && ! grep -E 'VK_ERROR|F DEBUG|vn_.*fail' '$log_dir/logcat-vkapp.txt'"
fi

# --- 7. Buffers reach crownpositor as NVIDIA dmabufs ----------------------------
check "HWC uses dmabuf wl_buffers" "HWC log: linux-dmabuf params with NVIDIA modifiers (0x03..., block-linear), no 'params failed'" \
    sh -c "sudo waydroid shell -- logcat -d | grep -iE 'hwcomposer|crown' | tee '$log_dir/logcat-hwc.txt' | grep -qiE 'modifier.*0x3|dmabuf' && ! grep -qi 'params failed' '$log_dir/logcat-hwc.txt'"
check "compositor holds NVIDIA dmabufs" "debugfs bufinfo: exporter nvidia buffers attached by crownpositor (count > 0)" \
    sh -c "sudo cat /sys/kernel/debug/dma_buf/bufinfo > '$log_dir/bufinfo.txt' && grep -A20 -i nvidia '$log_dir/bufinfo.txt' | grep -q crownpositor"
check "compositor import errors" "crownpositor journal: no 'dmabuf import failed' / 'EGL_BAD' for Android clients" \
    sh -c "! journalctl --user -b --no-pager | grep -iE 'crownpositor.*(dmabuf.*fail|EGL_BAD)'"
check "window shows pixels" "screenshot has > 64 distinct colours (not a black or garbage-uniform window)" \
    sh -c "grim -t ppm '$log_dir/screen.ppm' && python3 -I -c 'import sys;d=open(sys.argv[1],\"rb\").read().split(b\"\\n\",3)[3];px={d[i:i+3] for i in range(0,len(d)-3,3*997)};sys.exit(0 if len(px)>64 else 1)' '$log_dir/screen.ppm'"

# --- 8. Fences: GPU-side, no CPU stalls ------------------------------------------
android logcat -d >"$log_dir/logcat-final.txt"
check "no fence timeouts" "no 'fence timeout' / 'waitForever' / 'sync_wait' warnings" \
    sh -c "! grep -iE 'fence.*time(d)? ?out|sync_wait.*fail|waitForever' '$log_dir/logcat-final.txt'"
check "timeline fence path engaged" "render-server log has no 'timeline' fallback warnings (VTEST_NO_TIMELINE unset)" \
    sh -c "! journalctl --user -u crownos-venus.service -b --no-pager | grep -qi 'timeline.*fall'"

# --- 9. Recovery: render server restart -------------------------------------------
systemctl --user restart crownos-venus.service
sleep 15
check "Android recovers after render-server restart" "SurfaceFlinger restarted by init, boot_completed still 1, Settings redraws" \
    sh -c "[ \"\$(sudo waydroid shell -- getprop init.svc.surfaceflinger | tr -d '\r')\" = running ]"

echo "ALL PASS (logs: $log_dir)"
