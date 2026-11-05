#!/usr/bin/env bash
# Assembles the CrownOS Android overlay bundle from the HWC and agent build
# outputs, writes its manifest.json and, given a minisign key, signs it and
# packs bundle-<version>.tar.zst for the crownos-android package.
#
#   overlay/build-bundle.sh --version V [--key SECRET_KEY] [--out DIR]
#                           [--whiteouts FILE] [--setup BINARY]
#
# Inputs, overridable through the environment:
#   HWC_OUT    ~/.cache/crownos-android/hwc/out/x86_64
#              vendor/lib64/hw/hwcomposer.waydroid.so (crownos-waydroid-hwc ndk/build.sh)
#   AGENT_OUT  ~/.cache/crownos-android/agent/out
#              crown-agent.jar, crown-agent.rc, CrownHome.apk and the overlay
#              *.apk files (android/agent/build.sh, android/overlays/build.sh)
# Output: DIR/<version>/ (DIR defaults to ~/.cache/crownos-android/bundle).
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(dirname -- "$here")
cache=${XDG_CACHE_HOME:-$HOME/.cache}/crownos-android
hwc_out=${HWC_OUT:-$cache/hwc/out/x86_64}
agent_out=${AGENT_OUT:-$cache/agent/out}
out=$cache/bundle
version= key= whiteouts=
setup=(cargo run --quiet --locked --manifest-path "$repo/Cargo.toml" -p crownos-android-setup --)

die() { printf 'build-bundle: %s\n' "$*" >&2; exit 1; }

while (($#)); do
    case $1 in
        --version) version=${2:?}; shift 2 ;;
        --key) key=${2:?}; shift 2 ;;
        --out) out=${2:?}; shift 2 ;;
        --whiteouts) whiteouts=${2:?}; shift 2 ;;
        --setup) setup=("${2:?}"); shift 2 ;;
        *) sed -n '2,16p' "${BASH_SOURCE[0]}" >&2; exit 2 ;;
    esac
done
[[ $version =~ ^[0-9A-Za-z][0-9A-Za-z._-]*$ ]] || die "--version needs a version such as 2026.11.0"

hwc=$hwc_out/vendor/lib64/hw/hwcomposer.waydroid.so
jar=$agent_out/crown-agent.jar
rc=$agent_out/crown-agent.rc
home=$agent_out/CrownHome.apk
require() { [[ -f $1 ]] || die "missing $1; $2"; }
require "$hwc" "build it with crownos-waydroid-hwc's ndk/build.sh"
require "$jar" "build it with android/agent/build.sh"
require "$rc" "android/agent/build.sh puts android/init/crown-agent.rc next to the jar"
require "$home" "build it with android/overlays/build.sh"
mapfile -t overlays < <(find "$agent_out" -maxdepth 1 -type f -name '*.apk' ! -name CrownHome.apk | sort)
((${#overlays[@]})) || die "no overlay APKs in $agent_out; build them with android/overlays/build.sh"
[[ -z $whiteouts || -f $whiteouts ]] || die "missing whiteout list $whiteouts"
[[ -z $key || -f $key ]] || die "missing signing key $key"
[[ -z $key ]] || command -v minisign >/dev/null || die "signing needs minisign (pacman -S minisign)"

tree=$out/$version
staging=$out/.$version.partial
rm -rf "$staging"
install -D -m 0644 "$hwc" "$staging/vendor/lib64/hw/hwcomposer.waydroid.so"
install -D -m 0644 "$jar" "$staging/system/framework/crown-agent.jar"
install -D -m 0644 "$rc" "$staging/system/etc/init/crown-agent.rc"
for apk in "${overlays[@]}"; do
    install -D -m 0644 "$apk" "$staging/system/product/overlay/${apk##*/}"
done
install -D -m 0644 "$home" "$staging/system/product/app/CrownHome/CrownHome.apk"
[[ -z $whiteouts ]] || install -m 0644 "$whiteouts" "$staging/whiteouts.txt"

"${setup[@]}" make-manifest --root "$staging" --version "$version" --pins "$here/pins.json" >/dev/null
if [[ -n $key ]]; then
    minisign -S -s "$key" -m "$staging/manifest.json" -x "$staging/manifest.json.minisig" \
        -t "crownos-android bundle $version"
fi
rm -rf "$tree"
mv "$staging" "$tree"

if [[ -z $key ]]; then
    echo "built $tree (unsigned: setup refuses it until it is signed with --key)"
    exit 0
fi
tar -C "$out" --sort=name --owner=0 --group=0 --numeric-owner --mtime=@0 \
    -I 'zstd -19 -T0' -cf "$out/bundle-$version.tar.zst" "$version"
echo "built $tree and $out/bundle-$version.tar.zst"
