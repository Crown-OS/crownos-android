#!/usr/bin/env bash
# Checks the files in rootfs/ without installing them or needing root:
#   - systemd-analyze verify on every unit, the drop-in included, against a
#     staged root whose CrownOS executables are stubs;
#   - the polkit rule, run in node against the requests it must and must not grant;
#   - the polkit action and the firewalld zone with xmllint;
#   - channels.cfg read the way Waydroid reads it.

set -euo pipefail

packaging=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
rootfs=$packaging/rootfs
scratch=$(mktemp -d "${TMPDIR:-$HOME/.cache}/crownos-android-check.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT

stub_executables=(usr/lib/crownos-android/crownos-android-setup usr/bin/crown-androidd usr/bin/waydroid)

stage_root() {
    local stub
    cp -r -- "$rootfs/." "$scratch/root/"
    cp -rn -- /usr/lib/systemd/system/. "$scratch/root/usr/lib/systemd/system/"
    for stub in "${stub_executables[@]}"; do
        install -Dm755 /dev/null "$scratch/root/$stub"
    done
}

verify_units() {
    local unit
    systemd-analyze verify --root="$scratch/root" crownos-android-setup.service waydroid-container.service
    mkdir -p -- "$scratch/user"
    for unit in "$rootfs"/usr/lib/systemd/user/*; do
        sed "s#=/usr/#=$scratch/root/usr/#" "$unit" >"$scratch/user/${unit##*/}"
    done
    systemd-analyze --user verify "$scratch"/user/*
}

check_polkit_rule() {
    node - "$rootfs/usr/share/polkit-1/rules.d/50-crownos-android.rules" <<'EOF'
const { readFileSync } = require("node:fs");
const vm = require("node:vm");
const YES = "yes", NOT_HANDLED = "not handled";
let rule;
vm.runInNewContext(readFileSync(process.argv[2], "utf8"),
    { polkit: { Result: { YES, NOT_HANDLED }, addRule: (added) => { rule = added; } } });
const request = (unit, verb, local = true, active = true, id = "org.freedesktop.systemd1.manage-units") =>
    rule({ id, lookup: (key) => ({ unit, verb })[key] }, { local, active });
const cases = [
    [request("waydroid-container.service", "start"), YES],
    [request("waydroid-container.service", "stop"), YES],
    [request("crownos-android-setup.service", "start"), YES],
    [request("crownos-android-setup.service", "stop"), YES],
    [request("waydroid-container.service", "restart"), NOT_HANDLED],
    [request("sshd.service", "start"), NOT_HANDLED],
    [request("waydroid-container.service", "start", true, false), NOT_HANDLED],
    [request("waydroid-container.service", "start", false, true), NOT_HANDLED],
    [request("waydroid-container.service", "start", true, true, "org.freedesktop.login1.reboot"), NOT_HANDLED],
];
const failed = cases.filter(([got, want]) => got !== want).length;
if (failed) { console.error(`${failed} polkit rule case(s) failed`); process.exit(1); }
EOF
}

check_xml() {
    xmllint --noout "$rootfs/usr/share/polkit-1/actions/org.crownos.android.policy" \
        "$rootfs/usr/lib/firewalld/zones/crownos-android.xml"
}

check_channels() {
    python3 -I - "$rootfs/usr/share/waydroid-extra/channels.cfg" <<'EOF'
import configparser, sys
channels = configparser.ConfigParser()
channels.read(sys.argv[1])
section = channels["channels"]
# Any channel this file names can be initialised by every local user without
# polkit, so it must be one that can never be fetched.
for key in ("system_channel", "vendor_channel"):
    host = section[key].removeprefix("https://").split("/", 1)[0]
    if not section[key].startswith("https://") or not host.endswith(".invalid"):
        sys.exit(f"{key} must point at an unresolvable https host (*.invalid)")
for key in ("rom_type", "system_type"):
    section[key]
EOF
}

mkdir -p -- "$scratch/root"
stage_root
verify_units
check_polkit_rule
check_xml
check_channels
printf 'packaging checks passed\n'
