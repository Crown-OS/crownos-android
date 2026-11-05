# Venus tier packaging

On NVIDIA-only machines, Android renders through a patched Mesa venus driver. It talks over a vtest socket to a patched host virglrenderer, which drives NVIDIA's own Vulkan driver. The design and evidence are in `docs/decisions/s4-venus.md`. This directory builds and installs that chain as the `crownos-android-venus` package.

| File | Purpose |
|---|---|
| `build.sh`, `steps/*.sh` | Build every binary from pinned sources |
| `pins.env` | Source URLs, commits and sha256 values, the NDK revision and the release version |
| `patches/<project>/*.patch` | Our copy of the patch set, applied in order with `patch -Np1 --fuzz=0` |
| `crownos-android-render.service` | User unit for `venus::RENDER_SERVICE`, started by `crown-androidd` |
| `70-crownos-android-udmabuf.rules` | Gives the seat user `uaccess` to `/dev/udmabuf` |

## Building

```sh
packaging/venus/build.sh
```

The build needs:
- `curl git gzip meson ninja patch pkg-config glslangValidator sha256sum tar zstd`;
- a `python3` that imports `mako` and `yaml`;
- NDK r27c (`27.2.12479018`) at `$ANDROID_NDK`, default `~/Android/Sdk/ndk/27.2.12479018`.

It writes to `$WORK`, default `~/.cache/crownos-android/venus`. Downloads are cached in `$DOWNLOADS` and verified on every run. The source trees and build dirs are rebuilt from scratch each run. `JOBS` defaults to 4. On the dev laptop the whole build takes 53 s.

Output:

```
$WORK/out/overlay/vendor/lib64/hw/vulkan.virtio.so      patched venus ICD, x86_64
$WORK/out/overlay/vendor/lib/hw/vulkan.virtio.so        patched venus ICD, x86
$WORK/out/overlay/vendor/lib64/libgbm_mesa_wrapper.so   vtest gralloc backend, x86_64
$WORK/out/overlay/vendor/lib/libgbm_mesa_wrapper.so     vtest gralloc backend, x86
$WORK/out/host/virgl_test_server                        vtest server (--venus)
$WORK/out/host/virgl_render_server                      venus render worker
$WORK/out/host/libvirglrenderer.so.1                    patched libvirglrenderer
$WORK/out/licenses/                                     Mesa, virglrenderer, minigbm and waydroid-nvidia licences
$WORK/out/SHA256SUMS
$WORK/dist/crownos-android-venus-<release>.tar.zst      the tarball the PKGBUILD pins
```

The `overlay/` half is exactly `venus::overlay_files()`.

**Reproducibility.** The four guest files are byte-identical to the S4 spike artifacts. The release tarball is byte-identical across runs with the same toolchain: sorted entries, fixed owners and modes, and `SOURCE_DATE_EPOCH` from `pins.env`. Some things are embedded in the output, so they must not change between rebuilds that should match:
- the NDK, meson, glslang and host gcc versions;
- the relative path from a build dir to its source tree (`__FILE__` strings).

**Host build.** The host files are built for `/usr/lib/crownos-android/venus`:
- the compiled-in render server path is `/usr/lib/crownos-android/venus/virgl_render_server`;
- an `$ORIGIN` RUNPATH keeps `virgl_test_server` on our `libvirglrenderer.so.1`. Arch's virglrenderer stays installed for QEMU.

## Sources and patches

| Project | Pin | Patches | Origin |
|---|---|---|---|
| Mesa | `a8ce4d829191939d8db4381a09dadcffdf6b5b7c` (26.2.0-devel, 2026-07-10) | 0001 sync_fd export over vtest, 0002 dma-buf import over vtest, 0003 WIP: AHB memory steering, LINEAR imports, UMA flags, timeline fences, ASTC emulation | lithaway/waydroid-nvidia `7c5fbc28` |
| libdrm | 2.4.133 | none (Mesa's meson wrap) | freedesktop release |
| virglrenderer | `dc35e4db03144f81637c5ad061f61d3334b078fe` (after 1.3.0, 2026-07-06) | 0001 sync_file export, 0002 dma-buf import blob, 0003 listen backlog, 0004 WIP: `VCMD_RESOURCE_ALLOC_GPU` and global-priority retry, 0005 the allocator sources 0004 compiles | same |
| minigbm (Waydroid) | `16844babfbd83d7c206ca22256257cbf26ca7de1` | 0001 adds the vtest gralloc backend `vtest_wrapper.c`, built against `gbm_mesa_wrapper.h` | same |

The fork patches are byte-identical in lithaway `7c5fbc28` and Shiro836/waydroid-nvidia `8ef23a2d`. Patches 0005 (virglrenderer) and 0001 (minigbm) are the fork's net-new files, unchanged, recast as patches so that every project is a pinned tarball plus a patch series.

The fork's `patches/mesa/BASE` file, and S4 after it, abbreviate the Mesa pin as `a8ce4d8f`. The real commit is `a8ce4d82…`, which the archive's embedded commit id confirms.

**Licences.** The Mesa and virglrenderer patches are MIT, like their upstreams. The minigbm patch adds an MIT file to a BSD-3-Clause tree. The fork's own notice is `patches/waydroid-nvidia-LICENSE`. `build.sh` copies it and the upstream licences into `licenses/`.

## Mesa version

We build the fork pin `a8ce4d82` (26.2.0-devel) and not a stable release. The reasons:
- **The patches are cut against it.** `git apply --reject` of the three patches rejects hunks in all of them on 26.0.5, the vendor image's Mesa. On 26.2.6 it still rejects about 11 hunks (`vn_queue`, `vn_renderer_vtest`, `vn_renderer.h`, `vn_device.h`, `vn_command_buffer`), because stable picked up venus changes after the branch point.
- **The guest must speak the host's protocol.** The guest patches use vtest commands 39 to 41, which the virglrenderer patches add at `dc35e4db`. The two sides must change together.
- **The ICD is self-contained.** All Mesa internals are static, and it NEEDs only `libdrm libhardware liblog libnativewindow libsync libm libdl libc`. It therefore does not have to match the vendor's 26.0.5 `libgallium` or `libgbm`.

Rebasing onto 26.2.6+ or 26.3 stable is the follow-up before release, estimated at half a day per rebase:
1. Check out the new base. Run `git am` on patches 0001 and 0002, then `git apply --3way` on 0003. Resolve the conflicts, commit, and `git format-patch` the series back into `patches/mesa/`.
2. Update `MESA_COMMIT`, `MESA_URL` and `MESA_SHA256`, and `LIBDRM_*` if the wrap changed.
3. Run `build.sh`, then the runtime test below.

virglrenderer is rebased the same way, but only together with the guest, because the protocol must match.

The guest ICD keeps the build-tree RUNPATH (`$ORIGIN/../../../subprojects/…`) that S4 shipped. Bionic skips the missing dirs. Switching to `meson install` drops it, and the first rebase does that.

## The render server

The user unit `crownos-android-render.service` runs:

```
virgl_test_server --venus --no-virgl --multi-clients \
  --socket-path %h/.local/share/waydroid/data/crownos/venus/venus.sock
```

- **Socket path.** It is the host side of `venus::SOCKET_GUEST`, because Waydroid binds `~/.local/share/waydroid/data` as Android's `/data`. The server makes the socket 0777, so app uids can connect. On the host it sits under the user's 0700 home.
- **Socket dir.** `ExecStartPre` creates `data/crownos/venus` with mode 0711.
  - This works when the desktop user owns `data`, which is the case for uid 1000, Android's `system`.
  - For any other uid, Android owns `data`. Root must then create `data/crownos` and `data/crownos/venus`, mode 0711, owned by the desktop user. That belongs in the setup service's `prepare-container` step.
- **ICD.** `VK_DRIVER_FILES` pins the NVIDIA ICD, so lavapipe never becomes device 0.
- **`--no-virgl`.** This flag skips host EGL/GL. The forks run without it. If the runtime test fails at the render server, drop it first.
- **Lifecycle.** The unit has `Restart=on-failure` and no `[Install]`. `crown-androidd` starts it before the session, stops it with the runtime, and restarts it after resume.

The udev rule tags `/dev/udmabuf` `uaccess`, so the server can back CPU-mappable gralloc buffers with udmabuf.

## NVIDIA driver matrix

`venus::detect` requires all of the following, otherwise the machine falls back to another tier:
- the compositor's boot VGA GPU is driven by `nvidia`;
- the **open** kernel module is loaded. The closed module cannot export dma-bufs, and the open module only binds Turing (RTX 20 / GTX 16) and newer;
- the driver major is at least `MIN_DRIVER_MAJOR` (595);
- `nvidia_drm.modeset=1`, without which there is no `VK_EXT_external_memory_dma_buf`;
- udmabuf is present.

| Branch | Status | Evidence |
|---|---|---|
| 615 (615.78.08, open module) | **Tested branch.** The build is verified, and the driver has every required Vulkan extension. The runtime chain is pending V3 on the RTX 4050 in dGPU MUX mode | S4 on the dev laptop, kernel 7.2.9 |
| 610 | Expected to work | lithaway recommends 610 |
| 595 to 609 | Expected to work, untested by us | lithaway: 595.71+ |
| 580 to 594 | Rejected by `MIN_DRIVER_MAJOR` | Shiro: venus "from 580"; documented sync_file/dma-buf defects on 580 |
| ≤ 575 | Rejected | Shiro: defects on 535 and 550 to 560. Anbox needs 545+ for `SYNC_FD`. Mesa `venus.rst` lists 570.86+ as tested |

### Verifying a new branch

1. **Module.** `cat /proc/driver/nvidia/version` must say `Open Kernel Module`, and `cat /sys/module/nvidia_drm/parameters/modeset` must say `Y`.
2. **Extensions.** The host driver must expose all 7 extensions the chain uses. This prints 7:
   ```sh
   VK_DRIVER_FILES=/usr/share/vulkan/icd.d/nvidia_icd.json vulkaninfo 2>/dev/null \
     | grep -oE 'VK_(EXT_external_memory_dma_buf|EXT_image_drm_format_modifier|EXT_queue_family_foreign|KHR_external_fence_fd|KHR_external_semaphore_fd|KHR_global_priority|KHR_timeline_semaphore)\b' \
     | sort -u | wc -l
   ```
3. **Runtime.** Run `spikes/s4-venus/runtime-test.sh` inside an NVIDIA-only crownpositor session, with this package installed.
   - The script predates the package and calls the unit `crownos-venus.service`. Substitute `crownos-android-render.service`.
   - Every check must pass: SF GLES on ANGLE/Venus, HWUI `Pipeline=Skia (Vulkan)`, the dma-buf reaching crownpositor, fences, and recovery after a render server restart.
4. **Record.** Add a row to the table above. A failing branch at or above the minimum needs a code change in `crates/venus`: raise `MIN_DRIVER_MAJOR`, or reject the branch explicitly.

### Known gaps

- **Video decode:** no hardware video decode. Android uses the software `c2.android.*` codecs.
- **ETC2:** not emulated in Vulkan, so Vulkan games show placeholder textures. GLES still gets ETC2 through ANGLE's CPU decode.
- **ASTC LDR:** emulated in compute (Mesa 0003). Readback of emulated images is unsupported.
- **FP16:** no `RGBA_FP16` gralloc buffers. dma-buf mmap readback is slow.
- **Performance:** socket serialization makes Venus the slowest NVIDIA path, about 4× slower than native in Shiro's numbers, with first-use shader stutter.
- **ANGLE:** v1 uses the image's ANGLE 2.1.20440 with `debug.renderengine.backend=skiagl`. If V3 fails on it, ship a CI-built ANGLE `c1a25085` and add its six files to `venus::overlay_files()`.
- **Hybrid laptops:** these stay on the iGPU tier, because NVIDIA buffers do not display on the iGPU.
