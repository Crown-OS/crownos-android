#!/usr/bin/env bash
# S2 root steps: put the out-of-tree hwcomposer.waydroid.so into the Waydroid
# vendor overlay, or take it out again. Nothing in the images is modified.
#
#   sudo ./root-steps.sh install [path/to/hwcomposer.waydroid.so]
#   sudo ./root-steps.sh restore      # back to the stock module from vendor.img
#   sudo ./root-steps.sh status
#
# Waydroid mounts /var/lib/waydroid/overlay/vendor as a lower layer above
# vendor.img and /var/lib/waydroid/overlay_rw/vendor as the writable upper layer
# (tools/helpers/images.py, mount_rootfs). A file in the overlay therefore
# shadows the image's copy, and a file in overlay_rw would shadow the overlay.
#
# Every step is idempotent. Backups live in /var/lib/waydroid/crownos-backup/s2:
#   hwcomposer.waydroid.so.stock      copy of the module inside vendor.img
#   hwcomposer.waydroid.so.overlay    an overlay file that was there before us
#   hwcomposer.waydroid.so.overlay_rw a runtime copy in overlay_rw that would win
set -euo pipefail

WAYDROID_DIR=/var/lib/waydroid
MODULE=vendor/lib64/hw/hwcomposer.waydroid.so
OVERLAY_FILE=$WAYDROID_DIR/overlay/$MODULE
UPPER_FILE=$WAYDROID_DIR/overlay_rw/$MODULE
VENDOR_IMAGE=$WAYDROID_DIR/images/vendor.img
BACKUP_DIR=$WAYDROID_DIR/crownos-backup/s2
STOCK_BACKUP=$BACKUP_DIR/hwcomposer.waydroid.so.stock
OVERLAY_BACKUP=$BACKUP_DIR/hwcomposer.waydroid.so.overlay
UPPER_BACKUP=$BACKUP_DIR/hwcomposer.waydroid.so.overlay_rw
INVOKING_HOME=$(getent passwd "${SUDO_USER:-root}" | cut -d: -f6)
DEFAULT_BUILT=$INVOKING_HOME/.cache/crownos-android/s2/out/x86_64/$MODULE

die() { echo "root-steps: $*" >&2; exit 1; }
sha() { sha256sum "$1" | cut -d' ' -f1; }

require_root() { [[ $(id -u) == 0 ]] || die "run with sudo"; }

# The overlay is read when the container mounts its rootfs; changing a lower
# layer under a mounted overlayfs is undefined, so the container must be down.
require_container_stopped() {
    if findmnt -rn "$WAYDROID_DIR/rootfs" > /dev/null; then
        die "Waydroid rootfs is mounted; run 'waydroid session stop' and 'sudo waydroid container stop' first"
    fi
}

warn_if_overlays_disabled() {
    if ! grep -qx 'mount_overlays = True' "$WAYDROID_DIR/waydroid.cfg"; then
        echo "root-steps: WARNING mount_overlays is not True in $WAYDROID_DIR/waydroid.cfg;" \
            "Waydroid ignores the overlay until it is re-enabled" >&2
    fi
}

backup_stock_module() {
    mkdir -p "$BACKUP_DIR"
    if [[ ! -f $STOCK_BACKUP ]]; then
        debugfs -R "dump /lib64/hw/hwcomposer.waydroid.so $STOCK_BACKUP" "$VENDOR_IMAGE" 2> /dev/null
        [[ -s $STOCK_BACKUP ]] || die "could not extract the stock module from $VENDOR_IMAGE"
        chmod 0644 "$STOCK_BACKUP"
    fi
}

# A regular file or a whiteout (0:0 char device) in overlay_rw hides the overlay.
move_aside_upper_copy() {
    if [[ -e $UPPER_FILE || -L $UPPER_FILE ]]; then
        [[ -e $UPPER_BACKUP ]] || cp -a "$UPPER_FILE" "$UPPER_BACKUP"
        rm -f "$UPPER_FILE"
        echo "moved $UPPER_FILE to $UPPER_BACKUP"
    fi
}

install_module() {
    local built=${1:-$DEFAULT_BUILT}
    [[ -f $built ]] || die "built module not found: $built"
    file -b "$built" | grep -q 'ELF 64-bit LSB shared object, x86-64' || die "$built is not an x86-64 shared object"
    require_container_stopped
    warn_if_overlays_disabled
    backup_stock_module

    if [[ -f $OVERLAY_FILE ]] && ! cmp -s "$OVERLAY_FILE" "$built" && [[ ! -f $OVERLAY_BACKUP ]]; then
        cp -a "$OVERLAY_FILE" "$OVERLAY_BACKUP"
        echo "saved existing overlay file to $OVERLAY_BACKUP"
    fi
    move_aside_upper_copy

    if cmp -s "$OVERLAY_FILE" "$built" 2> /dev/null; then
        echo "already installed: $OVERLAY_FILE"
    else
        install -D -m 0644 -o root -g root "$built" "$OVERLAY_FILE"
        echo "installed $built -> $OVERLAY_FILE"
    fi
    show_status
}

restore_module() {
    require_container_stopped
    backup_stock_module
    move_aside_upper_copy
    if [[ -f $OVERLAY_BACKUP ]]; then
        install -m 0644 -o root -g root "$OVERLAY_BACKUP" "$OVERLAY_FILE"
        echo "restored the previous overlay file from $OVERLAY_BACKUP"
    elif [[ -e $OVERLAY_FILE ]]; then
        rm -f "$OVERLAY_FILE"
        echo "removed $OVERLAY_FILE; vendor.img's stock module is active again"
    else
        echo "no overlay file; the stock module is already active"
    fi
    show_status
}

show_status() {
    [[ -f $STOCK_BACKUP ]] && echo "stock (vendor.img) sha256: $(sha "$STOCK_BACKUP")"
    if [[ -f $OVERLAY_FILE ]]; then
        echo "overlay           sha256: $(sha "$OVERLAY_FILE")  $(stat -c '%U:%G %a' "$OVERLAY_FILE")"
    else
        echo "overlay           : none (stock module active)"
    fi
    [[ -e $UPPER_FILE ]] && echo "overlay_rw        : $UPPER_FILE exists and wins over the overlay"
    true
}

require_root
case ${1:-} in
    install) install_module "${2:-}" ;;
    restore) restore_module ;;
    status) show_status ;;
    *) die "usage: $0 install [built.so] | restore | status" ;;
esac
