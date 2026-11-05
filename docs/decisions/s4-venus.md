# S4: Venus GPU acceleration on NVIDIA-only machines

Spike date: 2026-11-05. Plan reference: "Risks" item 4 of
`~/.claude/plans/create-complete-production-grade-transient-sunbeam.md`.
Machine: Ryzen laptop, AMD Phoenix iGPU (`renderD128`, amdgpu) + RTX 4050
(`renderD129`, NVIDIA Open Kernel Module 615.78.08, `nvidia_drm.modeset=Y`),
kernel 7.2.9, waydroid 1.6.3, LineageOS 20 images (vendor built 2026-04-28,
system 2026-04-03, security patch 2026-02-01).

## Question

Can an NVIDIA-only CrownOS machine run the single Waydroid session
GPU-accelerated through Venus, with these pieces:

- Mesa venus Vulkan on the Android side;
- ANGLE for GLES;
- a host virglrenderer render server.

The constraints are that Android-side files arrive only as Waydroid overlay files plus props, and the host adds only services. If not, NVIDIA-only machines ship with SwiftShader in v1.

## Criteria

| ID | Criterion |
|---|---|
| V1 | The chain works with no image rebuild: overlay files + `waydroid.cfg [properties]` + host services. |
| V2 | The Android-side Mesa venus build succeeds locally (NDK, x86_64, API 33). |
| V3 | The full chain runs: gralloc → ANGLE/Venus → SurfaceFlinger → HWC → crownpositor (needs hardware run). |
| V4 | Maintenance is acceptable: stock NVIDIA driver, no tie to the RM ABI. |

## Verdicts

| ID | Verdict | One-line reason |
|---|---|---|
| V1 | **GO** | Every guest piece is a `/vendor` library that the overlay can replace. Nothing in system.img changes. One LXC mount line (the NVIDIA render node) must be appended by our setup, because stock `gpu.py` drops NVIDIA nodes. |
| V2 | **GO** | Stock Mesa 26.0.5 venus: 15 s. Patched venus at the fork pin (26.2.0-devel `a8ce4d8f` + 3 patches): 20 s for x86_64 and 18 s for x86. vtest gralloc for both ABIs: under 1 s. Patched host virglrenderer: 12 s. All use NDK r27c, API 33, `-j4`. |
| V3 | **NEEDS-RUNTIME** | Nothing was started. `spikes/s4-venus/runtime-test.sh` has 29 checks (one optional) with expected values. |
| V4 | **GO, with a real patch burden** | The host uses NVIDIA's stock Vulkan ICD through public Vulkan only. There is no RM ABI, no glibc shim and no per-driver import list. The cost is about 3,000 out-of-tree lines that we would own: Mesa, virglrenderer, and the gralloc backend. |

**Decision:** Venus stays the v1 NVIDIA-only tier, conditional on V3 passing on
the RTX 4050 in dGPU-MUX mode. If V3 fails, the plan's fallback applies:
SwiftShader, plus a reason on the Settings page.

## Findings

### 1. The working chain (lithaway/waydroid-nvidia = Shiro836 venus mode)

```
 Android app ─ GLES ─▶ ANGLE (libGLESv2_angle.so, Vulkan backend)
 Android app ─ Vulkan ──────────────┐
 HWUI (skiavk) / SF RenderEngine ───┤
                                    ▼
             vendor/lib{,64}/hw/vulkan.virtio.so   (Mesa venus, PATCHED, VN_DEBUG=vtest)
                                    │ venus protocol + SCM_RIGHTS fds over a unix socket
 gralloc allocator@4.0 (minigbm_gbm_mesa)
   └ libgbm_mesa_wrapper.so (vtest backend) ── VCMD_RESOURCE_ALLOC_GPU ──┐
                                    ▼                                    ▼
 host: virgl_test_server --venus --multi-clients (PATCHED virglrenderer, desktop user)
        ├ virgl_render_server workers → NVIDIA libGLX_nvidia Vulkan → RTX GPU
        └ vtest_gpu_alloc: exportable VkImage, NVIDIA block-linear modifier → dmabuf fd
                                    │ the same dmabuf fd returns to Android
 SurfaceFlinger ─▶ hwcomposer.waydroid.so ─ zwp_linux_dmabuf (modifier 0x03…) ─▶ compositor on NVIDIA EGL
 fences: per-context DRM timeline syncobj shared once; sync_file exported locally (no socket round trip)
```

Both community repos carry byte-identical venus patches:

- `diff -rq` on `patches/{mesa,virglrenderer}` and `src/` shows no differences;
- lithaway @ `7c5fbc28` (2026-10-19) is the venus-only fork;
- Shiro836 @ `8ef23a2d` (2026-10-28) keeps venus as a fallback mode. Its new default is "nvidia-prolapse", which uses NVK on the RM interface.

Per link:

| Link | What the forks do (evidence) |
|---|---|
| **gralloc** | `ro.hardware.gralloc=minigbm_gbm_mesa` turns on the vendor's `android.hardware.graphics.allocator@4.0-service.minigbm_gbm_mesa`. Vendor init rc: `on early-init && property:ro.hardware.gralloc=minigbm_gbm_mesa`. Mapper 4.0 is passthrough, so it loads in every process, 32-bit ones included. minigbm `dlopen`s `libgbm_mesa_wrapper.so`. The forks replace that file with `src/minigbm-vtest/vtest_wrapper.c` (374 lines). It sends `VCMD_RESOURCE_ALLOC_GPU` (41) over the socket and receives a dmabuf fd plus a modifier. **Allocation is host-side.** GPU buffers are NVIDIA exportable `VkImage`s with block-linear modifiers. CPU-mappable buffers are NVIDIA-linear or udmabuf (`/dev/udmabuf` on the host, `vtest_gpu_alloc.c`). minigbm still `open()`s a DRM node (`gralloc.gbm.device` or `/dev/dri/renderD*`, `gbm_mesa_internals.cpp:212-226`) but ignores the fd (`vtest_dev_create: (void)fd`). |
| **Vulkan ICD** | Mesa venus `vulkan.virtio.so`, selected by `ro.hardware.vulkan=virtio`. The transport is vtest because `VN_DEBUG=vtest` is set. On Android `os_get_option("VN_DEBUG")` looks up the props `debug.mesa.vn.debug`, then `vendor.mesa.vn.debug`, then `mesa.vn.debug` (`src/util/os_misc.c:175-204`). The socket comes from `VTEST_SOCKET_NAME`, which maps to `mesa.vtest.socket.name`. The default would be `/tmp/.virgl_test`, but `/tmp` is a container tmpfs. The forks bind-mount `/run/…/waydroid-venus` to `/dev/venus` with a patched `lxc.py`. **We use `/data/crownos/venus/venus.sock`** (section 5). |
| **Stock venus over vtest is not enough** | Mesa 26.0.5 `vn_renderer_vtest.c:933-934` has `has_dma_buf_import = false; has_external_sync = false`, and lines 1073 and 1085 set `create_from_dma_buf = NULL` and `export_syncobj = NULL`. So there is no `VK_KHR_external_{fence,semaphore}_fd` (`vn_physical_device.c:1094-1171`), and gralloc/AHB buffers cannot be imported. HWUI asserts on `VK_KHR_external_semaphore_fd`, and the Android swapchain needs ANB import. **The vendor's own `vulkan.virtio.so` cannot drive Android over vtest. A patched ICD is mandatory.** |
| **Mesa patches** | Base `a8ce4d8f` (26.2.0-devel, 2026-07-10). 0001 adds sync_fd export over vtest (+83 lines). 0002 adds `bo_create_from_dma_buf` through `VCMD_RESOURCE_IMPORT_BLOB` with the fd sent by SCM_RIGHTS (+81). 0003 is WIP (+1,184) and covers: AHB memory steering, forcing `DRM_FORMAT_MODIFIER` tiling, stripping `INPUT_ATTACHMENT` on LINEAR imports, UMA flags, timeline-syncobj fences, semaphore sync_fd import, and ASTC-LDR emulation in compute (`vn_astc_emu.c`). |
| **Host renderer** | virglrenderer `dc35e4d` (2026-07-06, after the 1.3.0 tag) + 4 patches: sync_file export (+325), dmabuf-import blob (+101), listen backlog (+1), and WIP `VCMD_RESOURCE_ALLOC_GPU` plus a global-priority strip/retry (+1,206, with new `vtest/vtest_gpu_alloc.c`, 496 lines). It runs as `virgl_test_server --venus --multi-clients --socket-path …`, which spawns `virgl_render_server` workers. The server `chmod 0777`s the socket after bind (patched `vtest_server.c:470`). GPU choice: the first discrete device with `VK_EXT_external_memory_dma_buf` and `VK_EXT_image_drm_format_modifier`, or `VTEST_ALLOC_GPU`. |
| **GLES (ANGLE)** | `ro.hardware.egl=angle` loads `/vendor/lib{,64}/egl/lib{EGL,GLESv2,GLESv1_CM}_angle.so`. The forks ship ANGLE `c1a25085` ("2.1.28324"), built with gn `target_os="android"`, `angle_libs_suffix="_angle"`, `angle_enable_swiftshader=false`, NDK API 30, plus 2 performance patches (ReBAR buffers, NVK detection). They report that the older prebuilt ANGLE crashed in threaded RenderEngine and forced `skiagl`. Props: `debug.hwui.renderer=skiavk` and `debug.renderengine.backend=skiaglthreaded`. |
| **SF / HWC import → compositor** | SF and apps import gralloc buffers into Vulkan as dmabufs (`VCMD_RESOURCE_IMPORT_BLOB`). The host imports into the same NVIDIA device, so the buffers stay native. The HWC reads the minigbm handle (fds, strides, offsets, modifier) and builds `zwp_linux_buffer_params_v1`. Block-linear modifiers (vendor `0x03`) bind in NVIDIA EGL as `GL_TEXTURE_2D`. NVIDIA EGL imports **LINEAR only as `GL_TEXTURE_EXTERNAL_OES`**, so KWin and Mutter fail on it (Shiro HWC patches 0003 and 0010). The fallbacks in their HWC: per-format refusal of LINEAR, a `wl_shm` copy for undisplayable layers, SF composition for YUV, and all dmabuf planes passed (0015). A compositor on a different GPU (hybrid iGPU) cannot show these buffers (lithaway README; Shiro HWC 0014 adds a cross-GPU copy). |
| **Fences and sync** | sync_fd export, timeline variant: the render worker owns a timeline syncobj per context and passes the guest a **freshly opened** render-node fd once (`VCMD_TIMELINE_GET`). Sharing one `drm_file` between two processes cost about 38 ms per frame through `drm_file_update_pid`. Per frame the guest does `SYNCOBJ_TIMELINE_WAIT(WAIT_AVAILABLE)`, then `TRANSFER`, then `EXPORT_SYNC_FILE`, with zero socket operations. The fallback is a `VCMD_SYNC_EXPORT_SYNC_FILE` round trip. Imported acquire fences are forwarded as host semaphore imports instead of CPU `sync_wait`. Kernel ≥ 6.16 gives the single-ioctl chain import; older kernels fall back to a binary-syncobj transfer. |
| **Props (forks)** | `ro.hardware.egl=angle`, `ro.hardware.vulkan=virtio`, `mesa.vn.debug=vtest`, `mesa.vtest.socket.name=/dev/venus/venus.sock`, `debug.hwui.renderer=skiavk`, `debug.renderengine.backend=skiaglthreaded`, `persist.waydroid.use_subsurface=true`, `ro.surface_flinger.max_frame_buffer_acquired_buffers=3`, the phase offsets set to 0, and `waydroid.drm_device=<nvidia node>`, with a patched `gpu.py` that trusts it. Any `ro.hardware.gralloc` override is **removed**. |
| **Files added to the guest** | `vendor/lib{,64}/hw/vulkan.virtio.so` and `vendor/lib{,64}/egl/lib{EGL,GLESv1_CM,GLESv2}_angle.so` (both ABIs). `vendor/lib64/libgbm_mesa_wrapper.so` and `vendor/lib64/hw/hwcomposer.waydroid.so` (64-bit only). `system/bin/surfaceflinger`, patched only for refresh rates above 240 Hz. All are bind-mounted from `/var/lib/waydroid/nv/guest` by the patched `lxc.py`, with no image rebuild. Shiro's newer default mode, by contrast, installs **its own vendor.img**. |
| **Host packages and services** | Patched `virgl_test_server`, `virgl_render_server` and `libvirglrenderer.so.1` in a private libdir; a user unit `wd-venus.service`; the udev rule `KERNEL=="udmabuf", TAG+="uaccess"`; a patched waydroid that adds mounts, a socket preflight and `suspend_action=none`. |
| **Driver, GPU and module** | The open kernel module is required: the closed module has no dma-buf export (lithaway README), so Turing (RTX 20 / GTX 16) or newer. `nvidia-drm.modeset=1` is required: without it there is no `VK_EXT_external_memory_dma_buf`. lithaway says 595.71+ (610 recommended); Shiro's matrix says venus works "from 580". Anbox Cloud needs 545+ for `VK_EXTERNAL_FENCE_HANDLE_TYPE_SYNC_FD_BIT`. Mesa's `venus.rst` lists NVIDIA 570.86+ as tested. Our plan's ≥ 595 sits inside all of these. The local 615.78.08 `libnvidia-glcore` contains every required extension: `VK_EXT_external_memory_dma_buf`, `VK_EXT_image_drm_format_modifier`, `VK_EXT_queue_family_foreign`, `VK_KHR_external_fence_fd`, `VK_KHR_external_semaphore_fd`, `VK_KHR_global_priority` and `VK_KHR_timeline_semaphore`. |
| **Known gaps** | **ETC2:** not emulated in Vulkan, so Vulkan games show placeholder textures. ANGLE still decodes ETC2 for GLES on the CPU. **ASTC LDR:** emulated by compute in patch 0003; readback is unsupported. **Video decode:** none, so Android uses software `c2.android.*` codecs (NVDEC exists only in Shiro's native mode). **Other:** no `RGBA_FP16` gralloc buffers; dmabuf mmap readback is slow. Shiro rates venus as the slowest mode (about 4× slower than the native mode; 1.8 s first-use shader stutter in Minecraft). |

Anbox Cloud ([rendering architecture](https://canonical.com/anbox-cloud/docs/explanation/rendering-architecture/),
[how to enable VirGL](https://discourse.ubuntu.com/t/how-to-enable-virgl/42426),
[supported rendering resources](https://canonical.com/anbox-cloud/docs/reference/supported-rendering-resources/))
confirms the same shape commercially. Its public docs say:

- "the Venus driver from the Mesa project" on the Android side;
- "a renderer on the Ubuntu side based on the virglrenderer project";
- "ANGLE to layer OpenGL ES and EGL on top";
- HWC → Anbox runtime over Wayland;
- VirGL is the default for NVIDIA since 1.22, needs driver 545+ for SYNC_FD, and is supported on Turing, Ampere and Ada;
- "Mixing GPUs from different vendors is not supported".

The docs do not describe their gralloc, transport or fence mechanism.

Mesa [`docs/drivers/venus.rst`](https://gitlab.freedesktop.org/mesa/mesa/-/blob/main/docs/drivers/venus.rst)
documents vtest as "the simplest way to test Venus" (`virgl_test_server --venus`,
`VN_DEBUG=vtest`). For Android it requires `VK_EXT_external_memory_dma_buf`,
`VK_EXT_image_drm_format_modifier` and `VK_EXT_queue_family_foreign` from the host driver.
vtest is a test transport upstream. The production paths are virtio-gpu (crosvm, QEMU).
Everything in the forks' patches is beyond upstream.

### 2. What the pinned images already contain

These were read with `debugfs -R "ls -l …"` and `dump`, without mounting. Copies are in
`~/.cache/crownos-android/s4/img/vendor/`.

| Item | vendor.img | system.img |
|---|---|---|
| ANGLE | **Yes**, both ABIs: `lib64/egl/libEGL_angle.so` (688,712 B), `libGLESv2_angle.so` (8,205,128 B), `libGLESv1_CM_angle.so`, plus `lib/egl/*` for x86. Version string "2.1.20440 git hash: 353815e0d958" (Android 13-era AOSP ANGLE), built with SwiftShader | None (only `libEGL.so`, `libGLESv2.so` and `libvulkan.so` loaders) |
| Venus ICD | **Yes**, `lib64/hw/vulkan.virtio.so` (1,439,168 B) and `lib/hw/vulkan.virtio.so`. **Mesa 26.0.5**, with vtest compiled in (`/tmp/.virgl_test`, `VTEST_SOCKET_NAME`, the `vtest` flag), but without dma-buf import or sync export (section 1) | — |
| Mesa | 26.0.5: `libgallium_dri.so`, `libEGL_mesa`, `libgbm_mesa`, LLVM 22; Vulkan ICDs `intel`, `intel_hasvk`, `radeon`, `lvp`, `pastel`, `virtio` | — |
| gralloc | `gralloc.gbm.so` (waydroid gbm, which upstream waydroid selects when it finds a DRI node), `gralloc.minigbm_gbm_mesa.so`, `libminigbm_gralloc_gbm_mesa.so`, `allocator@4.0-service.minigbm_gbm_mesa`, `mapper@4.0-impl.minigbm_gbm_mesa` (both ABIs), `libgbm_mesa_wrapper.so` (both ABIs) | — |
| Vendor venus dependencies | `libdrm`, `liblog`, `libcutils`, `libc++`, `libnativewindow`, `libsync`, `libhardware`, `mapper@4.0`, `libgralloctypes`, `libhidlbase`, `libutils` (AOSP-tree build with the IMapper4 u_gralloc) | — |

Consequence: SwiftShader-free GLES (ANGLE) is already in the image. For Venus, only the transport-capable ICD and the gralloc backend must be replaced.

### 3. Host virglrenderer

The installed package is `virglrenderer 1.3.0-2.1` (cachyos-extra-znver4; Arch `extra` has 1.3.0-2). It already has
**venus and the render server**:

- `/usr/lib/virgl_render_server`;
- `vkr_*` dispatch and `VK_MESA_venus_protocol` in `libvirglrenderer.so.1.11.0`;
- `virgl_test_server` usage includes `--venus`.

It is **not sufficient** for this chain: 1.3.0 (2026-02-10) predates the fork base `dc35e4d`
and has none of `VCMD_SYNC_EXPORT_SYNC_FILE` (39), `VCMD_RESOURCE_IMPORT_BLOB` (40) or
`VCMD_RESOURCE_ALLOC_GPU` (41). I therefore built the patched renderer:

- source `virglrenderer-dc35e4d` (tarball sha256 `cf2c4ebf…961a5e`), all 4 fork patches applied cleanly, plus `vtest_gpu_alloc.{c,h}`;
- `meson -Dvenus=true -Drender-server-worker=process -Dplatforms=egl -Dbuildtype=release`, `ninja -j4`;
- **12 s**, installed to `~/.cache/crownos-android/s4/host/` (`bin/virgl_test_server`, `libexec/virgl_render_server`, `lib/libvirglrenderer.so.1.11.0`).

It was not installed system-wide and not run. Note that the binary resolves `libvirglrenderer.so.1` to
`/usr/lib`, so the unit must set `LD_LIBRARY_PATH` (or we build with an rpath).
Do not replace Arch's package, because QEMU and `rutabaga-ffi` depend on it.

### 4. Android-side builds (NDK r27c 27.2.12479018, API 33, `-j4`)

The recipe is `spikes/s4-venus/build-mesa-android.sh`. It is Mesa `docs/android.rst` verbatim:

- `-Dplatforms=android -Dplatform-sdk-version=33 -Dandroid-stub=true -Dandroid-libbacktrace=disabled`;
- `-Dvulkan-drivers=virtio -Dgallium-drivers= -Degl=disabled -Dallow-fallback-for=libdrm`;
- `--wrap-mode=nodownload`, with the hash-verified `libdrm-2.4.123.tar.xz` placed in packagecache;
- Python deps (`mako`, `pyyaml`) come from a private venv.

| Build | Source | Result | Time |
|---|---|---|---|
| Stock venus, parity with vendor | `mesa-26.0.5.tar.xz` (sha256 `d229c993…023368`, matches the release notes) | `libvulkan_virtio.so` ELF64 x86-64, "for Android 33", 1,682,368 B unstripped. NEEDED is only `libdrm libcutils libhardware liblog libnativewindow libsync libm libdl libc`. u_gralloc backends: cros_api, fallback, libdrm, qcom | **15 s** total (10 s ninja, 260 steps) |
| **Patched venus, fork pin (the chain's ICD)** | Mesa `a8ce4d8f` (26.2.0-devel, GitLab archive sha256 `5f00da40…070293`) + 0001 0002 0003. All three apply cleanly (`git apply`). libdrm 2.4.133 wrap verified | x86_64: 2,159,048 B (1,826,344 B stripped). x86: 1,990,576 B (1,703,728 B stripped). NEEDED `libdrm libhardware liblog libnativewindow libsync libm libdl libc`. Contains the patched paths (`semaphore sync_fd import failed`, `VN_NO_ASTC_EMU`, `VTEST_NO_TIMELINE`). The only x86/x86_64 difference is blake3 asm | **x86_64 20 s** (13 s ninja, 264 steps); **x86 18 s** (10 s ninja, 260 steps) |
| vtest gralloc backend | `vtest_wrapper.c` + minigbm `16844ba` `gbm_mesa_wrapper.h` | `libgbm_mesa_wrapper.so` x86_64 and x86, NEEDED `liblog libdl libc`, exports `get_gbm_ops` | < 1 s each |

**Version choice: Mesa 26.2.x, built from the fork pin now and rebased onto 26.2.6+ before shipping.**

- The ICD is self-contained. All Mesa internals are static, and only Android system libraries are NEEDED. So it does not have to match the vendor's 26.0.5 `libgallium`/`libgbm`, which this tier does not use.
- What has to match is the virglrenderer patch set (versioned vtest commands), which is cut against `a8ce4d8f`.
- Trial applies (`git apply --reject`) of the fork patches: 26.0.5 rejects hunks in all three (vn_renderer_vtest, vn_android, vn_image, vn_physical_device, vn_renderer.h). 26.2.6 still rejects about 11 hunks (vn_queue, vn_renderer_vtest, vn_renderer.h, vn_device.h, vn_command_buffer), because stable picked up venus changes after the branch point.
- So the spike builds the exact pin. Production should rebase onto the current 26.2 or 26.3 stable, an estimated half day per rebase.

Using the NDK route instead of the AOSP tree changes one behaviour. u_gralloc talks to minigbm through the
**gralloc0 CrOS `perform` API** (`gralloc.minigbm_gbm_mesa.so`, through `hw_get_module`) rather than IMapper4.
That API returns fds, strides, offsets and the modifier, which is what venus needs.

### 5. ANGLE options

| Source | Version | Pros | Cons |
|---|---|---|---|
| **Image ANGLE** (already in vendor.img) | 2.1.20440 / `353815e0` | Zero files and zero work. Updated with the pinned image | Old (Android 13 era). The forks report a threaded-RenderEngine crash, so use `debug.renderengine.backend=skiagl`. Legacy LINEAR AHB tiling is handled by Mesa 0003. Not tested on Venus by anyone public |
| **Forks' release prebuilt** (Shiro v0.1.2 `guest-prebuilts`, 8.9 MB, SLSA-attested, sha256 `61899f56…` verified) | 2.1.28324 / `c1a25085` | Field-tested on venus (Turing to Blackwell). 6.5 MB `libGLESv2_angle` per ABI, no SwiftShader. Shiro states the recipe reproduces it byte for byte | Third-party binary from a project that has moved its default away from venus. Its version is in the release, not in our CI |
| **Google ANGLE APK** | Play-delivered | Signed, updatable | Only for apps (`angle_gl_driver_*` settings). SurfaceFlinger still loads `ro.hardware.egl`. Needs root or debuggable apps, is arm64-focused and depends on GMS. **Not viable** |
| **Source build in CrownOS CI** (pinned `c1a25085` + the 2 Shiro patches, gn args from `build/angle/args-x86_64.gn`) | any pin | Reproducible and owned. Can track ANGLE fixes | depot_tools + `gclient sync` with `target_os=['android']` pulls Chromium clang, the Android SDK/NDK, Rust and test suites. Expected well over 15 GB, so **not started here** (rule). Needs a build machine |

Recommendation: v1 starts on the **image ANGLE** with `skiagl`, and V3 A/B-tests it against the fork prebuilt. If the image
ANGLE fails V3, ship `c1a25085` built in CrownOS CI on a build machine. We take the fork binary only as a stop-gap
whose hash is pinned in the bundle manifest.

### 6. Integration fit with the CrownOS design

**Does the HWC get importable dmabufs on an NVIDIA-only host?** Yes:

- Every gralloc buffer is a host-NVIDIA dmabuf. Block-linear modifiers are used for GPU buffers; NVIDIA-linear or udmabuf for CPU buffers, NV12 always linear.
- The minigbm handle carries fds, offsets, strides and the modifier, which is exactly what our HWC needs for `zwp_linux_buffer_params_v1` (v4 feedback).
- crownpositor composites on NVIDIA through GBM, and Smithay's GLES renderer imports any format/modifier it does not render to as `TEXTURE_EXTERNAL_OES` (`smithay@e3d461a gles/mod.rs:1269-1351`). So LINEAR buffers that KWin and Mutter reject should display (NEEDS-RUNTIME).

Requirements for the L3 HWC lane, taken from the forks' fixes:

1. Intersect with the compositor's dmabuf feedback per (format, modifier).
2. Fall back to `wl_shm` for layers that cannot be imported.
3. Pass every plane, and compose YUV in SurfaceFlinger.
4. Never send an import the compositor did not advertise: `params` error 7 kills the connection.
5. The DRM node needed for `drm_syncobj` timelines is the mounted NVIDIA render node. NVIDIA supports timeline syncobjs, and they are device-agnostic.

Hybrid laptops stay on the iGPU Mesa tier, because NVIDIA buffers do not display on the iGPU (lithaway README, Shiro 0014).

**What `crownos-android-setup` must do (Venus tier):**

- **Detection** (all must hold, otherwise SwiftShader plus a Settings reason):
  - the boot_vga card, i.e. crownpositor's primary GPU (`udev::primary_gpu`), has `DRIVER=nvidia`;
  - `/proc/driver/nvidia/version` contains "Open Kernel Module". That also implies Turing or newer: the open module does not bind older GPUs;
  - driver major ≥ 595;
  - `/sys/module/nvidia_drm/parameters/modeset` is `Y`;
  - `/dev/udmabuf` exists (`CONFIG_UDMABUF`);
  - optionally, a Vulkan probe for the 7 extensions listed in section 1.
- **Overlay** (`/var/lib/waydroid/overlay`):
  - required: `vendor/lib{,64}/hw/vulkan.virtio.so` (patched venus) and `vendor/lib{,64}/libgbm_mesa_wrapper.so` (vtest gralloc, **both ABIs**, because the passthrough mapper loads in 32-bit app processes; the forks ship only 64-bit);
  - our `vendor/lib64/hw/hwcomposer.waydroid.so`;
  - optional: ANGLE `vendor/lib{,64}/egl/lib*_angle.so`;
  - no SurfaceFlinger binary.
  - Clear `overlay_rw/vendor` and `overlay_work/vendor` when the bundle changes.
- **Props** (`[properties]`, then `waydroid upgrade -o`):
  - `ro.hardware.gralloc=minigbm_gbm_mesa`. Stock props on NVIDIA-only give `default`, which is SwiftShader's gralloc;
  - `gralloc.gbm.device=/dev/dri/renderD<N>`;
  - `ro.hardware.egl=angle`;
  - `ro.hardware.vulkan=virtio`;
  - `mesa.vn.debug=vtest`;
  - `mesa.vtest.socket.name=/data/crownos/venus/venus.sock`. The gralloc wrapper reads only this name; Mesa also accepts the `debug.` and `vendor.` prefixes;
  - `debug.hwui.renderer=skiavk`;
  - `debug.renderengine.backend=skiagl` (or `skiaglthreaded` with the newer ANGLE).
- **Render node in the container:**
  - stock `gpu.py` drops nvidia nodes even when `drm_device` is set, and `config_nodes` is rewritten only by `init` and `upgrade`;
  - after each of those, append `lxc.mount.entry = /dev/dri/renderD<N> dev/dri/renderD<N> none bind,create=file 0 0`;
  - re-assert it from a `waydroid-container.service.d` `ExecStartPre=` drop-in;
  - the cleaner long-term fix is the upstreamable 3-line `gpu.py` change, "trust an explicit `drm_device`", the same as lithaway's patch.
- **Render server unit** (user unit, `crownos-venus.service`, ordered `Before=waydroid-session.service`):
  - `virgl_test_server --venus --no-virgl --multi-clients --socket-path <waydroid_data>/crownos/venus/venus.sock`;
  - `RENDER_SERVER_EXEC_PATH` set to our private libexec, `LD_LIBRARY_PATH` to our private libdir;
  - `VK_DRIVER_FILES=/usr/share/vulkan/icd.d/nvidia_icd.json`, so lavapipe never becomes device 0;
  - `--no-virgl` skips host EGL/GL; this is valid with `--venus` (`vtest_server.c:313-342`). The forks run without it, and V3 checks it.
- **Socket location:** `~/.local/share/waydroid/data` is bind-mounted as `/data` through `config_session`, so the socket path needs no LXC entry and survives `waydroid upgrade`.
  - Android's `/data` is owned by Android `system` (uid 1000). That is the desktop user only when the desktop user is uid 1000. So root creates `<data>/crownos/venus` with mode 0711, owned by the desktop user.
  - The server makes the socket 0777, so app uids can connect. Inside the container there is no SELinux.
  - On the host the path sits under the user's 0700 home, so other local users cannot reach it.
- **Host packaging (L10):** the patched virglrenderer goes into `/usr/lib/crownos-android/venus/`. Add the udev rule `KERNEL=="udmabuf", TAG+="uaccess"`.
  - After suspend/resume, restart the render server and let Venus clients reconnect: the plan already restarts Venus clients after resume.

The scripts for V3 are written and were not run:

- `spikes/s4-venus/root-steps.sh`: detection, overlay, props, `upgrade -o`, render-node mount and drop-in, host binaries, udev rule, user unit, socket directory.
- `spikes/s4-venus/runtime-test.sh`: render server, boot, props and nodes, gralloc backend, SF GLES string, `cmd gpu vkjson`, HWUI `Pipeline=Skia (Vulkan)`, an optional Vulkan APK, the dmabuf path into crownpositor (`/sys/kernel/debug/dma_buf/bufinfo`), fences, and recovery after a render-server restart. Each step lists its expected result.

On this laptop, run them with the display MUX in dGPU mode: `card0` (nvidia) owns `eDP-2`, so crownpositor's
boot_vga primary becomes NVIDIA. crownpositor has no primary-GPU override today.

## Recommended v1 path

1. **Host:** a `crownos-venus-renderer` package with virglrenderer at a pinned commit plus our copy of the 4 patches, in a private libdir, and the user unit described above. Keep Arch's virglrenderer for QEMU.
2. **Guest:** Mesa venus 26.2.x plus our copy of the 3 patches (x86_64 and x86), the vtest gralloc backend (x86_64 and x86), and our HWC. Use the image ANGLE first and a CI-built `c1a25085` ANGLE if V3 needs it. No system.img changes and no SurfaceFlinger replacement.
3. **Setup:** the detection, props, render-node mount and socket directory described above. The bundle manifest pins every binary by sha256, alongside the waydroid version and the image fingerprint (the minigbm wrapper ABI `struct gbm_ops` comes from the vendor image).
4. **Gate:** run `runtime-test.sh` on the RTX 4050 in dGPU mode. If any of SF GLES on ANGLE/Venus, HWUI Vulkan or the dmabuf in crownpositor fails, the tier ships disabled (SwiftShader plus a reason).

## Risks

- **Out-of-tree WIP patches.** Mesa 0003 and virglrenderer 0004 are labelled WIP. Upstream does not have them, and vtest is a test transport. The upstream projects (Shiro) have moved to an RM-ABI native driver, so we would be the maintainers of the venus patch set.
- **Rebase friction.** The patches already fail on 26.2.6 (about 11 hunks). Each Mesa or virglrenderer bump means re-resolving them.
- **Security.** Any process that can reach the socket can drive the GPU through the render server. This is mitigated by the 0700 home and the container-only bind, but it is not authenticated. udmabuf access is granted to the seat user.
- **NVIDIA driver regressions** in sync_file and dmabuf paths vary by branch: Shiro documents defects on 535, 550 to 560 and 580. We need a driver compatibility matrix in the bundle.
- **Performance.** Socket serialization makes Venus the slowest NVIDIA path (about 4× slower than native in Shiro's numbers), with shader-compile stutter. It is enough for apps but weak for games.
- **Feature gaps.** No hardware video decode, no ETC2 in Vulkan, no FP16 gralloc, slow readback.
- **Hybrid laptops** are not covered (iGPU tier). The dev laptop can test Venus only in dGPU MUX mode.
- **Silent overlay failure** (plan risk 5) would leave the stock venus active, which leads to an SF crash loop. Safe mode must cover it.

## Maintenance burden

| Item | Size | Cadence |
|---|---|---|
| Mesa venus patches | +1,348 / −34 lines (0001 to 0003) | rebase per Mesa stable used (about quarterly), plus CTS smoke |
| virglrenderer patches | +1,633 / −22 lines + 522-line allocator | rebase per pin bump (rare; the protocol must match the guest) |
| vtest gralloc backend | 374 lines | only when the Waydroid vendor changes `gbm_mesa_wrapper.h` |
| Waydroid integration | 1 config_nodes line + drop-in (or a 3-line upstream patch) | per waydroid release |
| ANGLE | 0 (image) or a CI source build | per ANGLE pin |
| NVIDIA | **no ABI coupling**: public Vulkan only, a ≥ 595 check, no per-driver data | none per driver release (contrast: Shiro's native and glibc-shim modes carry per-driver RM and glibc import tables) |

## Evidence index

- Spike artifacts: `spikes/s4-venus/{build-mesa-android.sh,root-steps.sh,runtime-test.sh}`.
- Staged artifacts in the `root-steps.sh` `ARTIFACTS` layout, with `SHA256SUMS`: `~/.cache/crownos-android/s4/artifacts/`. Guest: patched venus x86_64 and x86, vtest gralloc x86_64 and x86. Host: patched virglrenderer. Peak cache use is 1.6 GB.
- Builds and logs: `~/.cache/crownos-android/s4/{build,host,logs}/`. Sources: `~/.cache/crownos-android/s4/src/`. Extracted image files: `~/.cache/crownos-android/s4/img/vendor/`.
- Repos: [lithaway/waydroid-nvidia](https://github.com/lithaway/waydroid-nvidia) @ `7c5fbc28` (`docs/architecture.md`, `docs/transport-design.md`, `docs/fence-timeline-design.md`, `packaging/aur/waydroid-nvidia-bin/*`, `patches/*`); [Shiro836/waydroid-nvidia](https://github.com/Shiro836/waydroid-nvidia) @ `8ef23a2d` (`docs/architecture.md`, `angle/README.md`, `hwcomposer/patches/0003,0010,0014,0015`).
- Mesa `docs/drivers/venus.rst`, `docs/android.rst`, `src/virtio/vulkan/vn_renderer_vtest.c`, `src/util/os_misc.c` (26.0.5).
- Waydroid 1.6.3 `tools/helpers/{gpu.py,lxc.py,images.py}`; minigbm `16844ba` `gbm_mesa_driver/gbm_mesa_internals.cpp`.
