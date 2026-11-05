#!/usr/bin/env bash
# S4 Venus tier: root provisioning for an NVIDIA-only CrownOS machine.
# NOT executed by the spike. A human runs it once, as root, after
# `waydroid init`, with Android stopped:
#
#   sudo DESKTOP_USER=<user> ARTIFACTS=<dir> ./root-steps.sh
#   sudo DESKTOP_USER=<user> ./root-steps.sh revert    # undo everything below
#
# ARTIFACTS layout (produced by the spike builds, see docs/decisions/s4-venus.md):
#   guest/vendor/lib64/hw/vulkan.virtio.so      patched Mesa 26.2.6 venus, x86_64
#   guest/vendor/lib/hw/vulkan.virtio.so        patched Mesa 26.2.6 venus, x86
#   guest/vendor/lib64/libgbm_mesa_wrapper.so   vtest gralloc backend, x86_64
#   guest/vendor/lib/libgbm_mesa_wrapper.so     vtest gralloc backend, x86
#   guest/vendor/lib{,64}/egl/lib*_angle.so     OPTIONAL newer ANGLE (default: image ANGLE)
#   host/bin/virgl_test_server  host/libexec/virgl_render_server  host/lib/libvirglrenderer.so.1
#
# Every step prints what it changed. Re-running is safe.
set -euo pipefail

die() { echo "root-steps: $*" >&2; exit 1; }
[[ $(id -u) == 0 ]] || die "run as root"
action="${1:-install}"
user="${DESKTOP_USER:?set DESKTOP_USER}"
uid=$(id -u "$user") || die "unknown user $user"
home=$(getent passwd "$user" | cut -d: -f6)

readonly WAYDROID=/var/lib/waydroid
readonly CFG=$WAYDROID/waydroid.cfg
readonly OVERLAY=$WAYDROID/overlay
readonly NODES=$WAYDROID/lxc/waydroid/config_nodes
readonly HOST_DIR=/usr/local/lib/crownos-android/venus
readonly SOCKET_GUEST=/data/crownos/venus/venus.sock
readonly SOCKET_HOST_DIR=$home/.local/share/waydroid/data/crownos/venus
readonly MIN_DRIVER_MAJOR=595
readonly OVERLAY_FILES=(
    vendor/lib64/hw/vulkan.virtio.so vendor/lib/hw/vulkan.virtio.so
    vendor/lib64/libgbm_mesa_wrapper.so vendor/lib/libgbm_mesa_wrapper.so
    vendor/lib64/egl/libEGL_angle.so vendor/lib64/egl/libGLESv2_angle.so vendor/lib64/egl/libGLESv1_CM_angle.so
    vendor/lib/egl/libEGL_angle.so vendor/lib/egl/libGLESv2_angle.so vendor/lib/egl/libGLESv1_CM_angle.so
)
readonly PROPS=(
    ro.hardware.gralloc gralloc.gbm.device ro.hardware.egl ro.hardware.vulkan
    mesa.vn.debug mesa.vtest.socket.name debug.hwui.renderer debug.renderengine.backend
    ro.opengles.version
)

# ---------------------------------------------------------------------------
# revert: remove every file, prop, mount line, unit and rule `install` adds,
# so the machine goes back to the stock Mesa or SwiftShader tier.
# ---------------------------------------------------------------------------
revert() {
    waydroid status 2>/dev/null | grep -q "Session:.*RUNNING" && die "stop the Waydroid session first"
    local rel
    for rel in "${OVERLAY_FILES[@]}"; do
        if [[ -f $OVERLAY/$rel ]]; then
            rm -f "$OVERLAY/$rel"
            echo "   removed overlay /$rel"
        fi
    done
    rm -rf "$WAYDROID/overlay_rw/vendor" "$WAYDROID/overlay_work/vendor"
    PROPS="${PROPS[*]}" python3 -I - "$CFG" <<'EOF'
import configparser, os, sys
path = sys.argv[1]
cfg = configparser.ConfigParser()
cfg.optionxform = str
cfg.read(path)
if cfg.has_section("properties"):
    for key in os.environ["PROPS"].split():
        if cfg.remove_option("properties", key):
            print(f"   removed prop {key}")
with open(path, "w") as f:
    cfg.write(f)
EOF
    waydroid upgrade -o
    [[ -f $NODES ]] && sed -i '\|/dev/dri/renderD[0-9]* dev/dri/renderD[0-9]* none bind,create=file 0 0|d' "$NODES"
    rm -f /etc/systemd/system/waydroid-container.service.d/crownos-venus.conf
    rmdir --ignore-fail-on-non-empty /etc/systemd/system/waydroid-container.service.d 2>/dev/null || true
    systemctl daemon-reload
    rm -f /etc/udev/rules.d/70-crownos-venus-udmabuf.rules
    udevadm control --reload
    rm -f /etc/systemd/user/crownos-venus.service
    rm -rf "$HOST_DIR"
    rmdir --ignore-fail-on-non-empty /usr/local/lib/crownos-android 2>/dev/null || true
    echo "reverted. As $user: systemctl --user disable --now crownos-venus.service; systemctl --user daemon-reload"
}

case $action in
    revert) revert; exit 0 ;;
    install) ;;
    *) die "usage: $0 [install|revert]" ;;
esac
artifacts="${ARTIFACTS:?set ARTIFACTS}"

# ---------------------------------------------------------------------------
# 1. Detection: NVIDIA-only (compositor GPU is NVIDIA), open module, >= 595,
#    modeset, udmabuf. Expected on the dev laptop in dGPU MUX mode: all pass.
# ---------------------------------------------------------------------------
nvidia_render=""
for card in /sys/class/drm/card[0-9]*; do
    [[ -e $card/device/boot_vga && $(cat "$card/device/boot_vga") == 1 ]] || continue
    grep -qx DRIVER=nvidia "$card/device/uevent" || die "compositor GPU $(basename "$card") is not NVIDIA: use the Mesa or SwiftShader tier"
    nvidia_render=/dev/dri/$(basename "$(ls -d "$card"/device/drm/renderD* | head -1)")
done
[[ -n $nvidia_render ]] || die "no boot_vga NVIDIA GPU found"
echo "== NVIDIA render node: $nvidia_render"

grep -q "Open Kernel Module" /proc/driver/nvidia/version || die "closed NVIDIA kernel module: no dma-buf export, Venus tier unsupported"
driver=$(sed -n 's/.*Kernel Module for x86_64 *\([0-9.]*\).*/\1/p' /proc/driver/nvidia/version)
(( ${driver%%.*} >= MIN_DRIVER_MAJOR )) || die "driver $driver < $MIN_DRIVER_MAJOR"
[[ $(cat /sys/module/nvidia_drm/parameters/modeset) == Y ]] || die "nvidia_drm.modeset=1 required"
[[ -c /dev/udmabuf ]] || die "/dev/udmabuf missing (CONFIG_UDMABUF / modprobe udmabuf)"
echo "== open module $driver, modeset=Y, udmabuf present (open module implies Turing or newer)"

[[ -f $CFG ]] || die "waydroid not initialized"
waydroid status 2>/dev/null | grep -q "Session:.*RUNNING" && die "stop the Waydroid session first"

# ---------------------------------------------------------------------------
# 2. Overlay files. vendor/ in the overlay maps to /vendor in the container.
#    Clearing overlay_rw/overlay_work keeps stale copies from shadowing them.
# ---------------------------------------------------------------------------
install_overlay() {
    local rel=$1
    [[ -f $artifacts/guest/$rel ]] || return 0
    install -D -m 0644 -o root -g root "$artifacts/guest/$rel" "$OVERLAY/$rel"
    echo "   overlay: /$rel"
}
echo "== installing overlay files"
for rel in vendor/lib64/hw/vulkan.virtio.so vendor/lib/hw/vulkan.virtio.so \
           vendor/lib64/libgbm_mesa_wrapper.so vendor/lib/libgbm_mesa_wrapper.so \
           vendor/lib64/egl/libEGL_angle.so vendor/lib64/egl/libGLESv2_angle.so vendor/lib64/egl/libGLESv1_CM_angle.so \
           vendor/lib/egl/libEGL_angle.so vendor/lib/egl/libGLESv2_angle.so vendor/lib/egl/libGLESv1_CM_angle.so; do
    install_overlay "$rel"
done
[[ -f $OVERLAY/vendor/lib64/hw/vulkan.virtio.so && -f $OVERLAY/vendor/lib64/libgbm_mesa_wrapper.so ]] \
    || die "the patched venus ICD and gralloc backend are mandatory"
rm -rf "$WAYDROID/overlay_rw/vendor" "$WAYDROID/overlay_work/vendor"

# ---------------------------------------------------------------------------
# 3. Props in waydroid.cfg [properties]; `waydroid upgrade -o` merges them
#    into waydroid_base.prop (stock gpu.py gives NVIDIA-only hosts
#    gralloc=default, egl=swiftshader and no vulkan prop; these override it).
# ---------------------------------------------------------------------------
echo "== writing [properties]"
NVIDIA_RENDER=$nvidia_render SOCKET=$SOCKET_GUEST python3 -I - "$CFG" <<'EOF'
import configparser, os, sys
path = sys.argv[1]
cfg = configparser.ConfigParser()
cfg.optionxform = str
cfg.read(path)
if not cfg.has_section("properties"):
    cfg.add_section("properties")
props = {
    "ro.hardware.gralloc": "minigbm_gbm_mesa",
    "gralloc.gbm.device": os.environ["NVIDIA_RENDER"],
    "ro.hardware.egl": "angle",
    "ro.hardware.vulkan": "virtio",
    "mesa.vn.debug": "vtest",
    "mesa.vtest.socket.name": os.environ["SOCKET"],
    "debug.hwui.renderer": "skiavk",
    "debug.renderengine.backend": "skiagl",
    "ro.opengles.version": "196610",
}
for key, value in props.items():
    cfg.set("properties", key, value)
    print(f"   {key}={value}")
with open(path, "w") as f:
    cfg.write(f)
EOF
waydroid upgrade -o

# ---------------------------------------------------------------------------
# 4. NVIDIA render node into the container. Stock gpu.py drops nvidia nodes,
#    so append the mount after every init/upgrade (config_nodes is only
#    rewritten by those two). The HWC needs it for drm_syncobj, minigbm needs
#    an openable node. A waydroid-container drop-in re-applies it.
# ---------------------------------------------------------------------------
install -D -m 0755 /dev/stdin "$HOST_DIR/ensure-render-node" <<EOF
#!/bin/sh
set -eu
line="lxc.mount.entry = $nvidia_render ${nvidia_render#/} none bind,create=file 0 0"
grep -qxF "\$line" $NODES || echo "\$line" >> $NODES
EOF
"$HOST_DIR/ensure-render-node"
install -D -m 0644 /dev/stdin /etc/systemd/system/waydroid-container.service.d/crownos-venus.conf <<EOF
[Service]
ExecStartPre=$HOST_DIR/ensure-render-node
EOF
systemctl daemon-reload
echo "== config_nodes: $(grep -c "$nvidia_render" "$NODES") render-node entry"

# ---------------------------------------------------------------------------
# 5. Host render server (patched virglrenderer), udmabuf access, user unit.
# ---------------------------------------------------------------------------
install -D -m 0755 "$artifacts/host/bin/virgl_test_server" "$HOST_DIR/virgl_test_server"
install -D -m 0755 "$artifacts/host/libexec/virgl_render_server" "$HOST_DIR/virgl_render_server"
install -D -m 0755 "$artifacts/host/lib/libvirglrenderer.so.1" "$HOST_DIR/libvirglrenderer.so.1"

install -D -m 0644 /dev/stdin /etc/udev/rules.d/70-crownos-venus-udmabuf.rules <<'EOF'
SUBSYSTEM=="misc", KERNEL=="udmabuf", TAG+="uaccess"
EOF
udevadm control --reload
udevadm trigger --name-match=udmabuf

install -D -m 0644 /dev/stdin /etc/systemd/user/crownos-venus.service <<EOF
[Unit]
Description=CrownOS Venus render server for Android apps
Before=waydroid-session.service

[Service]
Environment=RENDER_SERVER_EXEC_PATH=$HOST_DIR/virgl_render_server
Environment=LD_LIBRARY_PATH=$HOST_DIR
Environment=VK_DRIVER_FILES=/usr/share/vulkan/icd.d/nvidia_icd.json
ExecStartPre=/usr/bin/test -d %h/.local/share/waydroid/data/crownos/venus
ExecStart=$HOST_DIR/virgl_test_server --venus --no-virgl --multi-clients --socket-path %h/.local/share/waydroid/data/crownos/venus/venus.sock
Restart=on-failure
RestartSec=1
Nice=-5

[Install]
WantedBy=default.target
EOF

# The data dir is owned by Android's system uid (1000), which is not always
# the desktop user, so root creates the socket dir and hands it over.
install -d -m 0711 -o "$uid" -g "$(id -g "$user")" "$(dirname "$SOCKET_HOST_DIR")" "$SOCKET_HOST_DIR"
echo "== socket dir: $SOCKET_HOST_DIR (container: $(dirname $SOCKET_GUEST))"

echo "done. As $user: systemctl --user daemon-reload && systemctl --user enable --now crownos-venus.service"
