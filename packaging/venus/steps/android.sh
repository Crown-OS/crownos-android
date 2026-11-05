# Guest builds with the NDK: the patched venus Vulkan ICD and the vtest
# gralloc backend, for both Android ABIs. Sourced by build.sh.

readonly ANDROID_ABIS=(x86_64 x86)
declare -rA ANDROID_TRIPLE=([x86_64]=x86_64-linux-android [x86]=i686-linux-android)
declare -rA ANDROID_CPU_FAMILY=([x86_64]=x86_64 [x86]=x86)
declare -rA ANDROID_CPU=([x86_64]=x86_64 [x86]=i686)
declare -rA VENDOR_LIBDIR=([x86_64]=lib64 [x86]=lib)

ndk_bin() {
    echo "$ANDROID_NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
}

ndk_clang() {
    echo "$(ndk_bin)/${ANDROID_TRIPLE[$1]}$ANDROID_API-clang"
}

# install_stripped <built library> <overlay path>
install_stripped() {
    install -D -m 0644 "$1" "$OUT/overlay/$2"
    "$(ndk_bin)/llvm-strip" "$OUT/overlay/$2"
}

# mesa_cross_file <abi> <build dir>: Mesa docs/android.rst, with host
# pkg-config files hidden so only the libdrm wrap is found.
mesa_cross_file() {
    local abi=$1 build=$2 bin cc
    bin=$(ndk_bin)
    cc=$(ndk_clang "$abi")
    cat <<EOF
[binaries]
ar = '$bin/llvm-ar'
c = ['$cc']
cpp = ['$cc++', '-fno-exceptions', '-fno-unwind-tables', '-fno-asynchronous-unwind-tables', '--start-no-unused-arguments', '-static-libstdc++', '--end-no-unused-arguments']
c_ld = 'lld'
cpp_ld = 'lld'
strip = '$bin/llvm-strip'
pkg-config = ['env', 'PKG_CONFIG_PATH=', 'PKG_CONFIG_LIBDIR=$build/empty-pkgconfig', '/usr/bin/pkg-config']

[host_machine]
system = 'android'
cpu_family = '${ANDROID_CPU_FAMILY[$abi]}'
cpu = '${ANDROID_CPU[$abi]}'
endian = 'little'

[properties]
needs_exe_wrapper = true
EOF
}

# build_venus_icd <abi> <patched mesa tree>
build_venus_icd() {
    local abi=$1 mesa=$2 build=$WORK/build/mesa-$1
    rm -rf "$build"
    mkdir -p "$build/empty-pkgconfig"
    mesa_cross_file "$abi" "$build" >"$build/android-$abi-api$ANDROID_API.ini"
    meson setup "$build" "$mesa" --cross-file "$build/android-$abi-api$ANDROID_API.ini" \
        --wrap-mode=nodownload \
        -Dbuildtype=release -Db_ndebug=true -Dstrip=true \
        -Dplatforms=android -Dplatform-sdk-version="$ANDROID_API" \
        -Dandroid-stub=true -Dandroid-libbacktrace=disabled \
        -Dvulkan-drivers=virtio -Dgallium-drivers= \
        -Degl=disabled -Dgles1=disabled -Dgles2=disabled -Dopengl=false \
        -Dglx=disabled -Dgbm=disabled -Dllvm=disabled \
        -Dvulkan-layers= -Dtools= -Dvideo-codecs= \
        -Dzstd=disabled -Dvalgrind=disabled -Dlibunwind=disabled -Dlmsensors=disabled \
        -Dallow-fallback-for=libdrm >"$build/setup.log" \
        || die "meson setup failed for mesa $abi, see $build/setup.log"
    ninja -C "$build" -j "$JOBS" src/virtio/vulkan/libvulkan_virtio.so
    install_stripped "$build/src/virtio/vulkan/libvulkan_virtio.so" \
        "vendor/${VENDOR_LIBDIR[$abi]}/hw/vulkan.virtio.so"
}

# build_vtest_gralloc <abi> <patched minigbm tree>
build_vtest_gralloc() {
    local abi=$1 driver=$2/gbm_mesa_driver build=$WORK/build/gralloc-$1
    rm -rf "$build"
    mkdir -p "$build"
    "$(ndk_clang "$abi")" -O2 -fPIC -shared -Wall -I"$driver" \
        -o "$build/libgbm_mesa_wrapper.so" "$driver/vtest_wrapper.c" \
        -llog -Wl,-soname,libgbm_mesa_wrapper.so
    install_stripped "$build/libgbm_mesa_wrapper.so" \
        "vendor/${VENDOR_LIBDIR[$abi]}/libgbm_mesa_wrapper.so"
}
