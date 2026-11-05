#!/usr/bin/env bash
# S1 root steps. Usage: sudo ./root-steps.sh [install|uninstall]
#
# Installs the S1 spike files into the Waydroid overlay and authorizes the host adb key.
# It never starts, stops or upgrades Waydroid: every change takes effect at the next
# `waydroid session start`. Run agent/build.sh and overlays/build.sh first, as your own user.
# Safe to run repeatedly.
set -euo pipefail

spike_dir="$(cd "$(dirname "$0")" && pwd)"
waydroid_dir=/var/lib/waydroid
overlay_dir="$waydroid_dir/overlay"
action="${1:-install}"

agent_jar="$spike_dir/agent/build/crown-agent-spike.jar"
init_rc="$spike_dir/init/crown-agent-spike.rc"
systemui_overlay="$spike_dir/overlays/build/CrownSpikeSystemUIOverlay.apk"
framework_overlay="$spike_dir/overlays/build/CrownSpikeFrameworkOverlay.apk"
home_apk="$spike_dir/overlays/build/CrownSpikeHome.apk"

# Overlay paths are relative to the container root: overlay/system/... appears as /system/...,
# and /product is a symlink to /system/product on this image.
overlay_targets=(
    "system/framework/crown-agent-spike.jar"
    "system/etc/init/crown-agent-spike.rc"
    "system/product/overlay/CrownSpikeSystemUIOverlay.apk"
    "system/product/overlay/CrownSpikeFrameworkOverlay.apk"
    "system/product/app/CrownSpikeHome/CrownSpikeHome.apk"
)

die() {
    echo "root-steps: $*" >&2
    exit 1
}

# Step 1: require root, and remember which desktop user owns the Waydroid data directory.
[[ $EUID -eq 0 ]] || die "run with sudo"
desktop_user="${SUDO_USER:-}"
[[ -n "$desktop_user" && "$desktop_user" != root ]] || die "run through sudo from your desktop user"
desktop_home="$(getent passwd "$desktop_user" | cut -d: -f6)"
android_data="$desktop_home/.local/share/waydroid/data"

# Step 2: refuse to run while the Android rootfs is mounted. Changing overlayfs lower layers
# under a live mount is undefined, and a running session would not see the files anyway.
if mountpoint -q "$waydroid_dir/rootfs"; then
    die "Waydroid session is running; stop it first with: waydroid session stop"
fi

# Step 3: make sure Waydroid mounts the overlay. images.py silently sets mount_overlays=False
# when an overlay mount fails once, which would hide every file installed below.
if grep -q '^mount_overlays = False' "$waydroid_dir/waydroid.cfg"; then
    sed -i 's/^mount_overlays = False/mount_overlays = True/' "$waydroid_dir/waydroid.cfg"
    echo "re-enabled mount_overlays in $waydroid_dir/waydroid.cfg"
fi

# Step 4: drop overlayfs upper and work dirs so stale copy-ups cannot shadow the new lower
# files. Waydroid recreates both directories on the next mount (helpers/mount.py).
clear_overlay_state() {
    rm -rf "$waydroid_dir/overlay_rw" "$waydroid_dir/overlay_work"
}

if [[ "$action" == uninstall ]]; then
    # Step 5u: remove every file this script installs, then reset overlay state.
    for target in "${overlay_targets[@]}"; do
        rm -f "$overlay_dir/$target"
    done
    rmdir --ignore-fail-on-non-empty "$overlay_dir/system/product/app/CrownSpikeHome" 2>/dev/null || true
    clear_overlay_state
    echo "uninstalled S1 spike files from $overlay_dir"
    exit 0
fi
[[ "$action" == install ]] || die "unknown action '$action' (install|uninstall)"

# Step 5: the build outputs must exist; this script never builds anything as root.
for artifact in "$agent_jar" "$init_rc" "$systemui_overlay" "$framework_overlay" "$home_apk"; do
    [[ -f "$artifact" ]] || die "missing $artifact; run agent/build.sh and overlays/build.sh first"
done

# Step 6: install the agent jar and its init service. init parses /system/etc/init/*.rc after
# Waydroid has mounted the overlay (container_manager.start: mount_rootfs, then lxc-start /init).
install -D -m 0644 -o root -g root "$agent_jar" "$overlay_dir/system/framework/crown-agent-spike.jar"
install -D -m 0644 -o root -g root "$init_rc" "$overlay_dir/system/etc/init/crown-agent-spike.rc"

# Step 7: install the runtime resource overlays as preinstalled product overlays, so idmap2
# grants them the product partition policy (neither target declares <overlayable>).
install -D -m 0644 -o root -g root "$systemui_overlay" \
    "$overlay_dir/system/product/overlay/CrownSpikeSystemUIOverlay.apk"
install -D -m 0644 -o root -g root "$framework_overlay" \
    "$overlay_dir/system/product/overlay/CrownSpikeFrameworkOverlay.apk"

# Step 8: install the blank HOME as a preinstalled product app.
install -D -m 0644 -o root -g root "$home_apk" \
    "$overlay_dir/system/product/app/CrownSpikeHome/CrownSpikeHome.apk"

# Step 9: reset overlayfs state after changing lower-layer files.
clear_overlay_state

# Step 10: authorize the host adb key for ro.adb.secure=1. /data/misc/adb is owned by Android
# system:shell (uid 1000, gid 2000); Waydroid does not remap uids, so numeric ids are used.
host_key="$desktop_home/.android/adbkey.pub"
adb_keys="$android_data/misc/adb/adb_keys"
if [[ -f "$host_key" && -d "$android_data/misc/adb" ]]; then
    touch "$adb_keys"
    if ! grep -qxF "$(cat "$host_key")" "$adb_keys"; then
        cat "$host_key" >> "$adb_keys"
        echo "authorized $host_key for adb"
    fi
    chown 1000:2000 "$adb_keys"
    chmod 0640 "$adb_keys"
else
    echo "skipped adb key: need $host_key (run 'adb start-server' once) and a booted Waydroid data dir"
fi

echo "installed S1 spike files into $overlay_dir; start a session to apply them"
