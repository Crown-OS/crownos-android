# S2: building the Waydroid hwcomposer without a LineageOS tree

Status: **decided for the build (H1–H3 GO); H4 still needs a runtime run**. Date: 2026-11-05.
Plan reference: `create-complete-production-grade-transient-sunbeam.md`, "Risks", item 2.

## Question

CrownOS must ship a modified `hwcomposer.waydroid.so` as a Waydroid overlay file
(`/var/lib/waydroid/overlay/vendor/lib64/hw/`), without custom Android images. A
LineageOS 20 tree (about 150 GB) does not fit on this machine. Can the HWC be
built out of tree, with the NDK compiler and the libraries of the pinned
images, so that it is ABI-identical to the stock module?

## Criteria and verdicts

| | Criterion | Verdict | Evidence in short |
|---|---|---|---|
| H1 | The HWC compiles out of tree | **GO** | Builds unchanged from upstream `lineage-20` with `-Wall -Werror` and no warnings. A cold build, downloads included, takes 1 min 42 s; rebuilding the HWC takes 5.6 s. Two builds from different directories are bit-identical. |
| H2 | Every undefined symbol resolves, with the correct namespace and versions | **GO** | All 290 of 290 imports resolve against the image libraries in vendor-namespace order, with 0 errors. 99 imports use `std::__1` and none use `__ndk1`. `DT_NEEDED` is identical to stock, in the same order. Rebuilt `vendor.waydroid.*` libraries match the image libraries: 8 of 8, with identical strong exports and 148 vtables/VTTs identical slot by slot. |
| H3 | The gralloc handle layout matches the pinned image | **GO** | The image's minigbm `allocate()` stores 9 fields, and those offsets plus `sizeof` (35 ints) match the header in `static_assert`s. The built and stock HWC read identical handle offsets. |
| H4 | The built .so loads and renders | **NEEDS-RUNTIME** | `root-steps.sh` and `runtime-test.sh` are ready (R1–R9 below). Nothing has been run here, because the rules forbid starting Waydroid. |

## Evidence

### Stock module (`vendor.img:/lib64/hw/hwcomposer.waydroid.so`)

- **File.** 508 512 bytes, sha256 `00934e97…68fded`. It was built from LineageOS 20
  (vendor build 2026-04-28, `TQ3A.230901.001`). The last HWC commit before that date
  is `ce61fbc` (2026-03-02); `de7f036` (2026-05-17) is not in the binary. Upstream
  HEAD `a575cfc` has 13 HWC commits after `ce61fbc`.
- **`DT_NEEDED` (22).** liblog, libutils, libcutils, libhardware, libhidlbase, libsync,
  libui, libdrm, libEGL, libGLESv2, vendor.waydroid.{clipboard@1.0, display@1.0/1.1/1.2,
  task@1.0, window@1.0/1.1/1.2}, libc++, libc, libm, libdl. libdrm and libhardware
  are listed although no symbol is imported from them, because Soong does not pass
  `--as-needed`.
- **Imports (290).** 86 from libc (`LIBC`, `LIBC_N`), 64 from libc++ (`std::__1`,
  including the old-ABI `__vector_base_common<true>`), 17 from libhidlbase, and the
  HIDL base methods plus `getService`/`registerAsService` from the vendor.waydroid
  libraries. The rest are libui (`GraphicBuffer`, `GraphicBufferMapper`), libutils
  (`RefBase`), libcutils (`property_*`, `atrace_*`), libsync and EGL/GLES. The
  module was linked against LLNDK stubs, so `egl*@LIBEGL`, `gl*@LIBGLESV2` and
  `sync*@LIBSYNC` carry version tags.
- **Static parts.** libwayland-client, libffi and libxkbcommon are statically
  linked, not taken from a vendor .so. Their public API is exported: 54 `wl_*`,
  24 `ffi_*`, 93 `xkb_*`. The generated `wayland-android` protocol code is compiled
  in ("code" mode exports 3 `android_wlegl_*` interfaces); the extension protocols
  use private-code. Exports with `wl_proxy_get_tag` but no `wl_proxy_marshal_flags`,
  plus `wl_display_add_protocol_logger_client`, identify AOSP `external/wayland`
  1.19 with its client-logger patch.
- **Other properties.** No RTTI, no exceptions, packed relocations (`ANDROID_RELA`
  and `RELR`), `BIND_NOW`, and `.gnu_debugdata` mini debuginfo. The checks use that
  debuginfo to find static functions.

### Where the libraries live in the images

The order below is the vendor linker namespace's search order. `make-sysroot.sh`
follows `DT_NEEDED` transitively and resolves 55 libraries (8.2 MB) in about 1 s,
with no mount.

| Location | Libraries the HWC needs |
|---|---|
| `vendor.img:/lib64` | libdrm, vendor.waydroid.* (8), android.hardware.graphics.composer@2.1 |
| `system.img:/system/apex/com.android.vndk.current/lib64` (flattened VNDK v33 APEX) | libhidlbase, libutils, libcutils, libhardware, libui, **libc++**, libbase, libgralloctypes, graphics HIDL/AIDL, … |
| `system.img:/system/lib64` (LLNDK) | liblog, libsync, libEGL, libGLESv2, libnativewindow, libvndksupport, … |
| `system.img:/system/apex/com.android.runtime/lib64/bionic` | libc, libm, libdl (`/system/lib64/libc.so` is a symlink there) |

### Headers

The sources are pinned in `ndk/sources.lock`, and `fetch-sources.sh` fails if any
of them drifts. They are shallow or sparse clones:

| Source | Repository and revision |
|---|---|
| Android base | `android-13.0.0_r75`, the tag LineageOS 20 is based on |
| LineageOS forks | `lineage-20.0` heads of bionic, libcxx, frameworks/native, hardware/libhardware, hardware/interfaces, system/core, logging, libhidl and libfmq |
| Waydroid repos | android_vendor_waydroid `lineage-20` (patches), android_external_libdrm and android_external_minigbm `lineage-18.1`, which are the repos in Waydroid's manifest |

The Waydroid patches that change headers or generated code are applied:

- **libhidl and libhwbinder:** host_hwbinder ABI.
- **hidl-gen:** host_hwbinder proxy code.
- **wayland-protocols:** fractional-scale.

The frameworks/native patches touch only SurfaceFlinger, input and binder headers,
none of which the HWC includes.

Size: 69 MB with `.git` (50 MB without). The first fetch took 1–2.5 minutes,
depending on the network.

Platform libc++: LineageOS `external/libcxx` (`_LIBCPP_VERSION 8000`, `std::__1`)
and AOSP `libcxxabi` headers. Bionic headers come from Android 13 as well, not
from NDK r27: NDK r27's `<ctype.h>` defines `isalnum_l` and the related functions
inline, which collides with this libc++'s `__posix_l_fallback.h`. The platform
build uses `-nostdlibinc` with bionic includes in the same way.

### HIDL route: build `hidl-gen` as a host tool

- **Options considered.** Hand-writing headers would also mean
  `android.hidl.base/manager` and `graphics.composer@2.1/common` types, with no way
  to prove that the vtables are right. No pre-generated tree matches Waydroid's
  patched hidl-gen. Building hidl-gen turned out to be small: `ndk/host/CMakeLists.txt`
  compiles the hidl-gen sources at android-13.0.0_r75 with the Waydroid patch, plus
  host libbase and liblog sources, bison/flex, OpenSSL SHA-256 and jsoncpp. That
  is 2 MB of output and 7 s of build time. It also lets CrownOS add HIDL versions
  later.
- **Generated headers.** `generate-interfaces.sh` produces 186 headers for
  vendor.waydroid.* (8 packages), android.hidl.base/manager, and
  graphics.common@1.0–1.2, composer@2.1 and mapper@4.0.
- **AIDL headers.** libui's `GraphicTypes.h` includes the AIDL NDK headers of
  `android.hardware.graphics.common` V3. These are generated from the frozen
  `aidl_api/.../3` snapshot (hash `e7e8b0bd…`) with SDK build-tools 34.0.0 `aidl`.
  The HWC uses them only as type names.
- **Interface hashes.** vendor.waydroid.* has no `current.txt`, so hidl-gen zeroes
  each package's own hash. The parent and IBase hashes are real. For example, the
  `display@1.2` chain is `[0, 6303e044… (1.1), 00cd2809… (1.0), ec7fd79e… (IBase)]`,
  and the 1.1 and 1.0 hashes appear verbatim in `vendor.waydroid.display@1.2.so`.
  Searching bytes is unreliable in general, because clang splits these constants
  into unaligned stores. The real proof is the next point.
- **ABI proof.** The build recompiles all 8 vendor.waydroid libraries from the
  generated sources and `check-hidl-abi.sh` compares them with `vendor.img`:
  - `DT_NEEDED`: identical.
  - Strong exports: 570 of 570 identical.
  - All 148 exported vtables, construction vtables and VTTs: identical slot by
    slot, where each slot is named by the symbol its relocation targets.

  The HWC's I*.h headers therefore describe exactly the ABI of the image
  libraries. The match required `-fvisibility-inlines-hidden`, which Soong passes
  globally. Without it, the inline destructor and `isRemote` slots differ.

### Build (H1)

- **Configuration.** NDK 27.2 clang with `--target=x86_64-linux-android33` and
  `ANDROID_STL=none`, plus Soong's vendor flags: `__ANDROID_VNDK__`,
  `__ANDROID_VENDOR__`, `-fno-exceptions`, `-fno-rtti`,
  `-fvisibility-inlines-hidden`, `-ftrivial-auto-var-init=zero`, `--gc-sections`,
  `-z now` and `--pack-dyn-relocs=android+relr`. Include directories mirror each
  library's vendor-variant export list. Shared libraries link in `Android.bp`
  order.
- **Static dependencies.** These are rebuilt from the same AOSP sources the image
  used: libffi (AOSP), libwayland-client 1.19 (AOSP, with the host
  `wayland-scanner` built from the same tree), libxkbcommon (AOSP, without
  `registry.c`, which the HWC never references), and 9 extension protocols.
- **Result.** 501 456 bytes, sha256 `4ad9307e…d55de5` at HEAD. Commit `ce61fbc`
  builds to 498 832 bytes (`6eb10267…`).

### Imports (H2), compared with stock

`check-symbols.sh` resolves imports the way bionic does: breadth-first over
`DT_NEEDED`, honouring GNU versions, with hidden definitions rejected for
unversioned references. As a control, it reports 290/290 with 0 errors on the
stock module itself.

Built from `ce61fbc`, the stock source, the import list equals stock except for:

- **22 imports without a version tag** (`egl*`, `gl*`, `sync_wait`, `sw_sync_*`).
  Soong links LLNDK stubs that define `LIBEGL`, `LIBGLESV2` and `LIBSYNC`. The real
  libEGL, libGLESv2 and libsync in the image define no versions, and bionic
  accepts both the versioned and the unversioned form against them
  (`check_symbol_version` returns true when the provider has no versym).
- **`+__register_atfork@LIBC`.** The NDK's `crtbegin_so.o` keeps `pthread_atfork`
  in `.text`, where `--gc-sections` cannot drop it. libc has exported this symbol
  since API 23.

At HEAD, the only further difference is that `configureRpcThreadpool` is no
longer imported, because upstream commit `4aee4c8` removed the call.

Exports:

- All C exports are identical: `wl_*`, `xkb_*`, `ffi_*`, `android_wlegl_*` and `HMI`.
- Against `ce61fbc`, 5 weak `std::__1` template instantiations exist only in stock
  and 1 only in the build, because clang 18 and clang 14 inline differently.

Allocation sizes inlined into the HWC are identical: `new GraphicBuffer` uses 0x100
bytes and `new GraphicBufferMapper` uses 0x10.

### Gralloc handle (H3)

The default gralloc is `minigbm_gbm_mesa`. The header comes from Waydroid
`android_external_minigbm` `lineage-18.1` at `e5a7769`, which is newer than the
image, so `check-gralloc-handle.sh` proves the layout instead of assuming it.

It reads `cros_gralloc_driver::allocate` in `vendor.img:/lib64/libminigbm_gralloc_gbm_mesa.so`:

| Field | Offset |
|---|---|
| width | 0x54 |
| height | 0x58 |
| format | 0x5c |
| tiling | 0x60 |
| format_modifier | 0x64 |
| use_flags | 0x6c |
| magic | 0x74 |
| pixel_stride | 0x78 |
| total_size | 0x90 |

`native_handle_create` reserves 35 ints, which means `sizeof` is 152.

- **Header check.** The offsets and size above are compiled as `static_assert`s
  against the header, and all of them pass. A mutated value makes the check fail.
- **Reader check.** The stock HWC (located through its `.gnu_debugdata` symbols)
  and the built HWC read the same offsets:
  - `create_buffer_cros`: `[0x5c, 0xc, 0x20, 0x30, 0x64]`, which are format,
    fds[0], strides[0], offsets[0] and format_modifier.
  - `get_buffer_metadata_cros`: `[0x54, 0x58, 0x78]`.

### Runtime (H4), not run here

- **Install.** `spikes/s2-hwc-build/root-steps.sh` stops if the Waydroid rootfs is
  mounted. It saves the stock module, from `vendor.img` through `debugfs`, plus any
  earlier overlay file and any copy in `overlay_rw` that would win over the
  overlay. It then installs the module with mode 0644, owned by root. Running it
  twice changes nothing, and `restore` and `status` are included. The whole flow
  was tested in a scratch directory.
- **Test.** `spikes/s2-hwc-build/runtime-test.sh <label>` checks, with results
  saved under `results/`:
  - R1: the overlay file is in place.
  - R2: boot completes within 240 s.
  - R3: the module's hash inside the container is correct.
  - R4: the composer service maps the module.
  - R5: logcat shows no link or HAL load errors.
  - R6: the composer pid stays stable for 15 s.
  - R7: SurfaceFlinger reports a display.
  - R8: a toplevel with app_id `Waydroid` appears (checked through lswt or hyprctl,
    otherwise you are asked).
  - R9: you confirm that the home screen renders and reacts to input.

  Run it as `stock` first as a baseline, then as `built`. Optionally, also run the
  `ce61fbc` build to separate toolchain effects from source changes.

## Reproducible steps

```sh
git clone -b lineage-20 https://github.com/waydroid/android_hardware_waydroid.git crownos-waydroid-hwc
crownos-waydroid-hwc/ndk/build.sh x86_64
#   fetch-sources.sh → make-sysroot.sh → host tools → generate-interfaces.sh
#   → NDK cross build → check-symbols.sh, check-hidl-abi.sh, check-gralloc-handle.sh
sudo crownos-android/spikes/s2-hwc-build/root-steps.sh restore && crownos-android/spikes/s2-hwc-build/runtime-test.sh stock
sudo crownos-android/spikes/s2-hwc-build/root-steps.sh install && crownos-android/spikes/s2-hwc-build/runtime-test.sh built
```

Inputs: NDK 27.2.12479018, SDK build-tools 34.0.0, cmake ≥ 3.27, clang, bison,
flex, OpenSSL, jsoncpp, expat, debugfs and the pinned images. Only `-j4` is used.
`build.sh arm64` is parameterised: every compile step was tried for arm64, but
linking needs arm64 images (`WAYDROID_IMAGES`).

## Size and time (x86_64, this machine, -j4)

| Item | Size | Time |
|---|---|---|
| Headers and host sources (`aosp/`) | 69 MB | 1–2.5 min to download |
| Sysroot (55 libraries) | 8.2 MB | 1 s |
| Host tools build | 7.5 MB | 6.8 s (24 s CPU) |
| Target build: HWC, static libraries, 8 HIDL libraries, generated headers | 42 MB | 5.6 s (18 s CPU) |
| Cold end-to-end `build.sh`, including all checks | ≈127 MB in `~/.cache/crownos-android/s2` | 1 min 42 s |

## Risks

1. **H4 is unproven.** The ABI evidence is strong, but codegen still differs (clang
   18 instead of clang 14), and HEAD carries 13 upstream HWC commits that the
   pinned image never ran. Running `ce61fbc` alongside HEAD separates the two.
2. **The build is tied to one image pair.** It is valid for this vendor/system
   pair only. A Waydroid image update can change minigbm, the HIDL libraries or the
   VNDK. CrownOS pins its images, but every image bump must re-run `build.sh`, and
   the setup service should refuse an HWC built for another vendor fingerprint.
   This fits the bundle-version handshake in plan risk 5.
3. **Linking against real libraries instead of LLNDK stubs.** The link would
   accept a non-LLNDK symbol that the platform build forbids. The runtime effect
   is nil, because namespaces filter by library, not by symbol. The 22 unversioned
   imports are a cosmetic difference from stock.
4. **AIDL headers come from aidl 34, not 33.** Only type names are used, and no
   symbol is imported from the AIDL libraries. A future HWC change that calls
   `GraphicBufferMapper` metadata APIs would need a recheck.
5. **Source drift.** `sources.lock` pins every revision and makes drift fatal.
   Branch heads can still disappear upstream, so CrownOS should mirror the 20
   pinned repositories (about 70 MB).
6. **Toolchain warnings.** The host tools need small accommodations for
   libstdc++ 15 (forced `<cstdint>`, `<cstring>` and `<algorithm>`). The
   cross-build itself compiles without a single warning.

## Fallback if a criterion turns out NO-GO

- **Toolchain suspect (stock works, `ce61fbc` fails).** Keep this route but swap
  in the platform compiler: `prebuilts/clang/host/linux-x86/clang-r450784d`, a
  sparse fetch of about 0.5 GB. Same sysroot, same headers, still no tree.
- **Source suspect (`ce61fbc` works, HEAD fails).** Bisect the 13 upstream
  commits. This is an HWC fix, not a build problem.
- **Both fail (ABI or behaviour not explained by the checks).** Fall back to plan
  risk 2: a LineageOS 20 tree on a build machine (about 150 GB) and
  `m hwcomposer.waydroid`, which builds one module, not an image. The user must
  be asked before that is set up.

## Produced

- Fork: `/e/CrownOS/crownos-waydroid-hwc`, not committed (`ndk/` is untracked). It
  contains:
  - Build and fetch: `build.sh`, `fetch-sources.sh`, `sources.lock`,
    `make-sysroot.sh`, `generate-interfaces.sh`, `CMakeLists.txt`, `cmake/*.cmake`,
    `host/CMakeLists.txt`.
  - Checks: `check-symbols.sh`, `check-hidl-abi.sh`, `check-gralloc-handle.sh`,
    `tools/{elf_symbols,hidl_abi,gralloc_layout}.py`.
  - `README.md`.
- Spike: `/e/CrownOS/crownos-android/spikes/s2-hwc-build/root-steps.sh` and `runtime-test.sh`.
- Cache: `~/.cache/crownos-android/s2/{aosp,sysroot/x86_64,stock/x86_64,build,out/x86_64,logs}`.
  The HEAD build is at `out/x86_64/vendor/lib64/hw/hwcomposer.waydroid.so` and the
  `ce61fbc` build at `out/x86_64-ce61fbc/...`.
