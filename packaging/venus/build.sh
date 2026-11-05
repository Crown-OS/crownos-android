#!/usr/bin/env bash
# Builds the Venus tier of CrownOS Android from pinned sources (pins.env) and
# the patches under patches/, reproducing the S4 spike chain:
#
#   overlay/vendor/lib{64,}/hw/vulkan.virtio.so      patched Mesa venus, x86_64 and x86
#   overlay/vendor/lib{64,}/libgbm_mesa_wrapper.so   vtest gralloc backend, x86_64 and x86
#   host/virgl_test_server, host/virgl_render_server, host/libvirglrenderer.so.1
#
# plus licenses/ and SHA256SUMS, in $WORK/out, and the release tarball in $WORK/dist.
#
# Env: ANDROID_NDK (default ~/Android/Sdk/ndk/$NDK_REVISION),
#      WORK (default ~/.cache/crownos-android/venus), DOWNLOADS (default $WORK/downloads),
#      JOBS (default 4).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$here/pins.env"
source "$here/steps/common.sh"
source "$here/steps/sources.sh"
source "$here/steps/android.sh"
source "$here/steps/host.sh"
source "$here/steps/release.sh"

ANDROID_NDK=${ANDROID_NDK:-$HOME/Android/Sdk/ndk/$NDK_REVISION}
WORK=${WORK:-${XDG_CACHE_HOME:-$HOME/.cache}/crownos-android/venus}
DOWNLOADS=${DOWNLOADS:-$WORK/downloads}
JOBS=${JOBS:-4}
OUT=$WORK/out
export SOURCE_DATE_EPOCH

main() {
    check_host_tools
    check_ndk
    mkdir -p "$DOWNLOADS"

    step "fetching pinned sources into $DOWNLOADS"
    local mesa_archive libdrm_archive virgl_archive minigbm_archive
    mesa_archive=$(fetch "mesa-$MESA_COMMIT.tar.gz" "$MESA_URL" "$MESA_SHA256")
    libdrm_archive=$(fetch "libdrm-$LIBDRM_VERSION.tar.xz" "$LIBDRM_URL" "$LIBDRM_SHA256")
    virgl_archive=$(fetch "virglrenderer-$VIRGL_COMMIT.tar.gz" "$VIRGL_URL" "$VIRGL_SHA256")
    minigbm_archive=$(fetch "minigbm-$MINIGBM_COMMIT.tar.gz" "$MINIGBM_URL" "$MINIGBM_SHA256")

    local mesa virgl minigbm abi
    mesa=$(source_tree mesa "$MESA_COMMIT")
    virgl=$(source_tree virglrenderer "$VIRGL_COMMIT")
    minigbm=$(source_tree minigbm "$MINIGBM_COMMIT")

    step "patching sources"
    unpack_patched "$mesa" "$mesa_archive" "$MESA_COMMIT" "$here/patches/mesa"
    install -D -m 0644 "$libdrm_archive" "$mesa/subprojects/packagecache/libdrm-$LIBDRM_VERSION.tar.xz"
    unpack_patched "$virgl" "$virgl_archive" "$VIRGL_COMMIT" "$here/patches/virglrenderer"
    unpack_patched "$minigbm" "$minigbm_archive" "$MINIGBM_COMMIT" "$here/patches/minigbm"

    rm -rf "$OUT"
    for abi in "${ANDROID_ABIS[@]}"; do
        step "venus ICD for $abi"
        build_venus_icd "$abi" "$mesa"
        step "vtest gralloc backend for $abi"
        build_vtest_gralloc "$abi" "$minigbm"
    done
    step "host render server"
    build_render_server "$virgl"

    step "release"
    install_licenses "$mesa" "$virgl" "$minigbm" "$here/patches"
    check_output_tree
    write_checksums
    cat "$OUT/SHA256SUMS"
    pack_release
}

main "$@"
