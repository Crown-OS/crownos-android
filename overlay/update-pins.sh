#!/usr/bin/env bash
# Writes overlay/pins.json: the waydroid package version and the LineageOS 20
# images a bundle is tested with, copied from Waydroid's OTA channels.
#
#   overlay/update-pins.sh [--system-datetime N] [--vendor-datetime N]
#                          [--waydroid VERSION] [--arch x86_64|arm64]
#
# The GAPPS system build is the one with --system-datetime. The VANILLA build
# is the one built on the same day, so both flavors share one tested base.
set -euo pipefail

ota=https://ota.waydro.id
system_datetime=1775208984
vendor_datetime=1777409843
waydroid=1.6.3
arch=x86_64
out=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/pins.json

die() { printf 'update-pins: %s\n' "$*" >&2; exit 1; }

while (($#)); do
    case $1 in
        --system-datetime) system_datetime=${2:?}; shift 2 ;;
        --vendor-datetime) vendor_datetime=${2:?}; shift 2 ;;
        --waydroid) waydroid=${2:?}; shift 2 ;;
        --arch) arch=${2:?}; shift 2 ;;
        *) sed -n '2,10p' "${BASH_SOURCE[0]}" >&2; exit 2 ;;
    esac
done
for tool in curl jq; do
    command -v "$tool" >/dev/null || die "$tool is required"
done

fetch() { curl -fsSL --retry 3 "$ota/$1" || die "could not fetch $ota/$1"; }
gapps=$(fetch "system/lineage/waydroid_$arch/GAPPS.json")
vanilla=$(fetch "system/lineage/waydroid_$arch/VANILLA.json")
vendor=$(fetch "vendor/waydroid_$arch/MAINLINE.json")

jq -n \
    --argjson gapps "$gapps" --argjson vanilla "$vanilla" --argjson vendor "$vendor" \
    --argjson system_datetime "$system_datetime" --argjson vendor_datetime "$vendor_datetime" \
    --arg waydroid "$waydroid" --arg arch "$arch" '
    def entry: {datetime, filename, id, romtype, size, url, version};
    def build_day: .filename | capture("-(?<day>[0-9]{8})-").day;
    def pick(builds; test; what): builds | map(select(test)) | first // error("no \(what)");
    pick($gapps.response; .datetime == $system_datetime; "GAPPS build \($system_datetime)") as $system
    | pick($vendor.response; .datetime == $vendor_datetime; "vendor build \($vendor_datetime)") as $vendor_build
    | pick($vanilla.response; build_day == ($system | build_day); "VANILLA build of \($system | build_day)") as $plain
    | {
        waydroid: $waydroid,
        images: [
            {flavor: "gapps", arch: $arch, system: ($system | entry), vendor: ($vendor_build | entry)},
            {flavor: "vanilla", arch: $arch, system: ($plain | entry), vendor: ($vendor_build | entry)}
        ]
    }' >"$out.tmp"
mv "$out.tmp" "$out"
echo "wrote $out"
