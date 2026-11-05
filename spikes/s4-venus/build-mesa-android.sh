#!/usr/bin/env bash
# Cross-build Mesa's venus Vulkan ICD (vulkan.virtio.so) for the Waydroid
# LineageOS 20 guest with the Android NDK, following Mesa's docs/android.rst.
#
#   build-mesa-android.sh <mesa-src> <build-dir>
#
# Env: NDK (default ~/Android/Sdk/ndk/27.2.12479018), ANDROID_ABI (x86_64|x86),
#      ANDROID_API (default 33 = Android 13), JOBS (default 4),
#      LIBDRM_TARBALL (verified libdrm wrap tarball, copied into packagecache).
set -euo pipefail

src="${1:?usage: build-mesa-android.sh <mesa-src> <build-dir>}"
build="${2:?usage: build-mesa-android.sh <mesa-src> <build-dir>}"
ndk="${NDK:-$HOME/Android/Sdk/ndk/27.2.12479018}"
abi="${ANDROID_ABI:-x86_64}"
api="${ANDROID_API:-33}"
jobs="${JOBS:-4}"
ndk_bin="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"

case "$abi" in
    x86_64) triple=x86_64-linux-android; cpu_family=x86_64; cpu=x86_64 ;;
    x86) triple=i686-linux-android; cpu_family=x86; cpu=i686 ;;
    *) echo "unsupported ANDROID_ABI=$abi" >&2; exit 1 ;;
esac
cc="$ndk_bin/$triple$api-clang"
[[ -x "$cc" ]] || { echo "missing $cc" >&2; exit 1; }

if [[ -n "${LIBDRM_TARBALL:-}" ]]; then
    install -Dm644 "$LIBDRM_TARBALL" "$src/subprojects/packagecache/$(basename "$LIBDRM_TARBALL")"
fi

mkdir -p "$build/empty-pkgconfig"
cross="$build/android-$abi-api$api.ini"
cat > "$cross" <<EOF
[binaries]
ar = '$ndk_bin/llvm-ar'
c = ['$cc']
cpp = ['$cc++', '-fno-exceptions', '-fno-unwind-tables', '-fno-asynchronous-unwind-tables', '--start-no-unused-arguments', '-static-libstdc++', '--end-no-unused-arguments']
c_ld = 'lld'
cpp_ld = 'lld'
strip = '$ndk_bin/llvm-strip'
pkg-config = ['env', 'PKG_CONFIG_PATH=', 'PKG_CONFIG_LIBDIR=$build/empty-pkgconfig', '/usr/bin/pkg-config']

[host_machine]
system = 'android'
cpu_family = '$cpu_family'
cpu = '$cpu'
endian = 'little'

[properties]
needs_exe_wrapper = true
EOF

if [[ ! -f "$build/build.ninja" ]]; then
    meson setup "$build" "$src" --cross-file "$cross" --wrap-mode=nodownload \
        -Dbuildtype=release -Db_ndebug=true -Dstrip=true \
        -Dplatforms=android -Dplatform-sdk-version="$api" \
        -Dandroid-stub=true -Dandroid-libbacktrace=disabled \
        -Dvulkan-drivers=virtio -Dgallium-drivers= \
        -Degl=disabled -Dgles1=disabled -Dgles2=disabled -Dopengl=false \
        -Dglx=disabled -Dgbm=disabled -Dllvm=disabled \
        -Dvulkan-layers= -Dtools= -Dvideo-codecs= \
        -Dzstd=disabled -Dvalgrind=disabled -Dlibunwind=disabled -Dlmsensors=disabled \
        -Dallow-fallback-for=libdrm
fi

start=$(date +%s)
ninja -C "$build" -j "$jobs" src/virtio/vulkan/libvulkan_virtio.so
echo "build seconds: $(( $(date +%s) - start ))"
file "$build/src/virtio/vulkan/libvulkan_virtio.so"
