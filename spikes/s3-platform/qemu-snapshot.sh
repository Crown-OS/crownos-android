#!/usr/bin/env bash
# Builds a reusable QEMU VM for functional tests: CrownOS installed unattended from an ISO, then
# Waydroid installed and initialized from images you provide. Not for performance numbers.
#
#   qemu-snapshot.sh build IMAGES_DIR [ISO]   IMAGES_DIR holds system.img and vendor.img;
#                                             ISO defaults to the newest in crownos-iso/out
#   qemu-snapshot.sh overlay NAME             a copy-on-write disk over the read-only base
#   qemu-snapshot.sh run NAME                 boots that overlay; serial log and QMP socket in the work dir
#
# The machine is install-test.sh's: q35, OVMF, virtio disk, autoinstall through fw_cfg. The
# autoinstall's kernel_args carry a one-shot systemd.run= hook that runs provision.sh (handed in
# through fw_cfg) on the first boot of the installed system; provision.sh removes the hook from
# the boot entry before it powers off, so the base boots clean.
#
# GPU_MODE=soft (default): virtio-vga without 3D, and Waydroid forced to SwiftShader through
#   [properties], because gpu.py would otherwise pick the virtio render node.
# GPU_MODE=virgl: virtio-vga-gl rendered on RENDER_NODE through egl-headless. gpu.py maps the
#   virtio_gpu render node to gralloc=gbm and egl=mesa (virgl); virtio_gpu has no entry in its
#   Vulkan table, so ro.hardware.vulkan stays unset and Venus is not used.

set -euo pipefail
shopt -s nullglob

crownos_iso=${CROWNOS_ISO_DIR:-/e/CrownOS/crownos-iso}
work=${S3_VM_DIR:-$HOME/.cache/crownos-android/s3/qemu}
firmware=${OVMF_DIR:-/usr/share/edk2/x64}
gpu_mode=${GPU_MODE:-soft}
render_node=${RENDER_NODE:-/dev/dri/renderD128}
install_timeout=${INSTALL_TIMEOUT:-1800}
provision_timeout=${PROVISION_TIMEOUT:-2400}
base=$work/base.qcow2
provision_fw_cfg=opt/org.crownos.s3/provision
provision_hook="systemd.run=\"/bin/sh -c 'udevadm settle; exec /bin/sh /sys/firmware/qemu_fw_cfg/by_name/$provision_fw_cfg/raw'\""

machine=(
    qemu-system-x86_64 -machine q35,accel=kvm:tcg -cpu max -m "${VM_MEMORY:-4G}" -smp "${VM_CPUS:-4}"
    -no-reboot
    -drive "if=pflash,format=raw,readonly=on,file=$firmware/OVMF_CODE.4m.fd"
)

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

newest_iso() {
    local iso newest=''
    for iso in "$crownos_iso"/out/*.iso; do
        [[ $iso -nt $newest ]] && newest=$iso
    done
    printf '%s' "$newest"
}

gpu_args() {
    case $gpu_mode in
        soft) printf '%s\n' -device virtio-vga -display none ;;
        virgl) printf '%s\n' -device virtio-vga-gl -display "egl-headless,rendernode=$render_node" ;;
        *) die "GPU_MODE must be soft or virgl" ;;
    esac
}

wait_for_exit() {
    local vm=$1 timeout=$2 waited
    for ((waited = 0; waited < timeout; waited += 5)); do
        kill -0 "$vm" 2>/dev/null || return 0
        sleep 5
    done
    kill "$vm" 2>/dev/null || true
}

write_provision_script() {
    cat >"$work/provision.sh" <<EOF
#!/bin/sh
# Runs once as root on the first boot of the installed system (kernel-command-line.service).
set -eu
done=0
say() { echo "crownos-s3: \$*" >/dev/console; }
trap '[ "\$done" = 1 ] || say "provision failed"; systemctl --no-block poweroff' EXIT

images=/var/lib/waydroid/images
mkdir -p "\$images"
cat /dev/disk/by-id/virtio-wd-system >"\$images/system.img"
cat /dev/disk/by-id/virtio-wd-vendor >"\$images/vendor.img"

if ! command -v waydroid >/dev/null; then
    systemctl start NetworkManager-wait-online.service || true
    pacman -Sy --needed --noconfirm waydroid
fi
waydroid init -f -i "\$images"
if [ "$gpu_mode" = soft ]; then
    cfg=/var/lib/waydroid/waydroid.cfg
    grep -q '^\[properties\]$' "\$cfg" || printf '\n[properties]\n' >>"\$cfg"
    sed -i '/^\[properties\]$/a ro.hardware.gralloc = default\nro.hardware.egl = swiftshader' "\$cfg"
    waydroid upgrade --offline
fi
systemctl enable waydroid-container.service

for entry in /efi/loader/entries/*.conf /boot/loader/entries/*.conf /etc/kernel/cmdline; do
    [ -f "\$entry" ] && sed -i -E 's/ ?systemd\.run="[^"]*"//; s/ ?systemd\.run_(success|failure)_action=[a-z]+//g' "\$entry"
done
sync
done=1
say "provision complete"
EOF
}

build() {
    local images=${1:?IMAGES_DIR} iso label
    iso=$(realpath -- "${2:-$(newest_iso)}")
    [[ -f $images/system.img && -f $images/vendor.img ]] || die "$images needs system.img and vendor.img"
    [[ -f $iso ]] || die "no ISO; build one in $crownos_iso first"
    [[ -e $base ]] && die "$base exists; remove $work to rebuild"

    mkdir -p -- "$work"
    truncate -s 32G "$work/disk.raw"
    printf 'label: gpt\n' | sfdisk --quiet "$work/disk.raw"
    cp -- "$firmware/OVMF_VARS.4m.fd" "$work/vars-base.fd"
    bsdtar -xf "$iso" -C "$work" --strip-components=3 \
        crownos/boot/x86_64/vmlinuz-linux-cachyos crownos/boot/x86_64/initramfs-linux-cachyos.img
    jq -n --arg args "console=ttyS0 systemd.show_status=1 $provision_hook systemd.run_success_action=none systemd.run_failure_action=none" \
        '{username: "crown", password: "crownos", hostname: "crownos-s3", kernel_args: $args}' >"$work/autoinstall.json"
    write_provision_script
    label=$(blkid -o value -s LABEL -- "$iso")

    printf 'installing %s\n' "${iso##*/}"
    timeout --foreground "$install_timeout" "${machine[@]}" -nic none -display none \
        -drive "if=pflash,format=raw,file=$work/vars-base.fd" \
        -drive "if=virtio,format=raw,file=$work/disk.raw" \
        -cdrom "$iso" -kernel "$work/vmlinuz-linux-cachyos" -initrd "$work/initramfs-linux-cachyos.img" \
        -append "archisobasedir=crownos archisolabel=$label console=ttyS0" \
        -fw_cfg "name=opt/org.crownos/autoinstall,file=$work/autoinstall.json" \
        -serial "file:$work/install.log" || die "install VM did not power off; see $work/install.log"
    grep -q 'crownos-installer: install complete' "$work/install.log" || die "install failed; see $work/install.log"
    qemu-img convert -O qcow2 "$work/disk.raw" "$base"
    rm -f -- "$work/disk.raw" "$work/vmlinuz-linux-cachyos" "$work/initramfs-linux-cachyos.img"

    printf 'provisioning Waydroid (GPU_MODE=%s)\n' "$gpu_mode"
    "${machine[@]}" -nic user,model=virtio-net-pci -display none \
        -drive "if=pflash,format=raw,file=$work/vars-base.fd" \
        -drive "if=virtio,format=qcow2,file=$base" \
        -drive "if=none,id=wdsystem,format=raw,readonly=on,file=$(realpath -- "$images/system.img")" \
        -device virtio-blk-pci,drive=wdsystem,serial=wd-system \
        -drive "if=none,id=wdvendor,format=raw,readonly=on,file=$(realpath -- "$images/vendor.img")" \
        -device virtio-blk-pci,drive=wdvendor,serial=wd-vendor \
        -fw_cfg "name=$provision_fw_cfg,file=$work/provision.sh" \
        -serial "file:$work/provision.log" &
    wait_for_exit $! "$provision_timeout"
    grep -q 'crownos-s3: provision complete' "$work/provision.log" || die "provisioning failed; see $work/provision.log"
    chmod a-w -- "$base" "$work/vars-base.fd"
    printf 'base ready: %s (GPU_MODE=%s)\n' "$base" "$gpu_mode"
}

overlay() {
    local name=${1:?NAME}
    [[ -f $base ]] || die "no base; run build first"
    qemu-img create -q -f qcow2 -b "$base" -F qcow2 "$work/$name.qcow2"
    install -m 0644 -- "$work/vars-base.fd" "$work/$name-vars.fd"
    printf '%s\n' "$work/$name.qcow2"
}

run() {
    local name=${1:?NAME}
    [[ -f $work/$name.qcow2 ]] || die "no overlay $name; run overlay $name first"
    mapfile -t gpu < <(gpu_args)
    exec "${machine[@]}" "${gpu[@]}" \
        -nic "user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:${SSH_PORT:-2222}-:22" \
        -drive "if=pflash,format=raw,file=$work/$name-vars.fd" \
        -drive "if=virtio,format=qcow2,file=$work/$name.qcow2" \
        -qmp "unix:$work/$name.qmp,server=on,wait=off" \
        -serial "file:$work/$name-serial.log"
}

command=${1:-}
shift || true
case $command in
    build) build "$@" ;;
    overlay) overlay "$@" ;;
    run) run "$@" ;;
    *) sed -n '2,20p' "${BASH_SOURCE[0]}" >&2; exit 2 ;;
esac
