# S3 platform baseline: kernel, binder, sw_sync, measurement kit

Status: offline half done 2026-11-05. Runtime half (baseline numbers, QEMU snapshot) waits for approval.

## Question

Does the kernel the CrownOS ISO ships give the one Waydroid session everything it needs without DKMS? Does it give a binder that avoids Waydroid #2422, and what replaces `CONFIG_SW_SYNC`? Are the scripts that measure the plan's budgets ready?

## Criteria

| id | criterion |
|---|---|
| K1 | The ISO kernel gives Waydroid a working binder without DKMS. |
| K2 | SW_SYNC is present, or there is a concrete HWC fork plan to replace it. |
| K3 | The measurement scripts are ready. |

## Verdicts

| id | verdict | one line |
|---|---|---|
| K1 | **GO for the 7.2/7.3 ISO; a decision is due before 7.4** | linux-cachyos builds the C binder and binderfs into the kernel, so #2422 cannot occur. Upstream 7.4 deletes the C binder, and linux-cachyos's ThinLTO plus BTF build cannot enable Rust, so a 7.4 linux-cachyos would have no binder at all. A boot on linux-cachyos still has to confirm K1 (NEEDS-RUNTIME). |
| K2 | **GO (plan); NEEDS-RUNTIME (vgem fence path)** | No candidate kernel has SW_SYNC. The stock HWC degrades to `-1` fences and works. The fork replaces sw_sync with drm_syncobj for acquire and vgem fences for the future release and present fences. Exporting the syncobj release point as a sync_file, as plan risk 3 words it, does not work. |
| K3 | **GO; numbers NEEDS-RUNTIME** | The scripts are written, syntax-checked and smoke-tested with Android stopped. Nothing that starts Android was run. |

## Evidence: kernel configs

Sources, all under `~/.cache/crownos-android/s3/configs/`:
- **7.2.7-1:** `linux-cachyos-7.2.7-1-x86_64`, the exact package in `crownos-2026.10.31-x86_64.iso`, taken from `crownos-iso/target/pkg-cache`. Its config came from `vmlinuz` with `extract-ikconfig`, because the matching headers are already gone from the mirror (404).
- **7.3.1-1:** `linux-cachyos-headers-7.3.1-1-x86_64`, the package the next ISO build resolves to. It came from mirror.cachyos.org, and its sha256 matched the `[cachyos]` db, whose signature `gpgv` checked against the CachyOS key F3B6…5A47.
- **LTS:** the CachyOS GitHub source config for `linux-cachyos-lts`, which is 6.18.50-based; the repo builds 6.18.60-1.
- **zen:** `/proc/config.gz` on the dev machine.

The installed `linux-cachyos 7.2.9-2` znver4 build matches 7.2.7-1 and 7.3.1-1 on every option below, so the dev machine can stand in for the ISO kernel at runtime. `spikes/s3-platform/kernel-check.sh` produced the table.

| option | cachyos 7.2.7-1 (ISO) | cachyos 7.3.1-1 (next ISO) | cachyos-lts 6.18 | zen 7.2.9 (dev) |
|---|---|---|---|---|
| ANDROID_BINDER_IPC (C) | y | y | y | n |
| ANDROID_BINDER_IPC_RUST | – | – | – | y |
| ANDROID_BINDERFS | y | y | y | – (Rust binderfs is unconditional) |
| ANDROID_BINDER_DEVICES | binder,hwbinder,vndbinder | same | same | "" |
| RUST | – (blocked by LTO+BTF) | – | y | y |
| SW_SYNC | n | n | n | n |
| SYNC_FILE, DMA_SHARED_BUFFER | y | y | y | y |
| DMABUF_HEAPS, _SYSTEM, _CMA, UDMABUF | y | y | y | y |
| PSI (PSI_DEFAULT_DISABLED=n) | y | y | y | y |
| ASHMEM | – (removed upstream in 5.18; Waydroid writes `sys.use_memfd=true`) | – | – | – |
| MEMFD_CREATE, SHMEM | y | y | y | y |
| page size | 4 KiB (PAGE_SHIFT=12) | 4 KiB | 4 KiB | 4 KiB |
| CGROUP_FREEZER (v1), MEMCG, PIDS, CPUSETS, CGROUP_BPF, CGROUP_DEVICE | y | y | y | y |
| USER_NS, NET_NS, PID_NS, IPC_NS, UTS_NS, SECCOMP_FILTER | y | y | y | y |
| VETH, BRIDGE | m | m | m | m |
| NF_TABLES (INET/IPV4 = y), NFT_NAT, NFT_MASQ, NF_NAT (NF_NAT_MASQUERADE=y) | m | m | m | m |
| iptables fallback: IP_NF_{IPTABLES,FILTER,NAT,MANGLE,TARGET_MASQUERADE}, NETFILTER_XT_{NAT,TARGET_MASQUERADE,TARGET_CHECKSUM}, NFT_COMPAT | m | m | m | m |
| DRM (syncobj is DRM core), DRM_KMS_HELPER | y | y | y | y |
| DRM_VGEM, DRM_VIRTIO_GPU, DRM_SCHED | m | m | m | m |
| FUSE_FS, EXT4_FS / OVERLAY_FS, BLK_DEV_LOOP, TUN | y / m | y / m | y / m | y / m |
| ZSWAP (DEFAULT_ON, zstd, shrinker on), LRU_GEN_ENABLED / ZRAM | y / m | y / m | y / m | y / m |
| MODULE_SIG=y, MODULE_SIG_FORCE / LOCK_DOWN_IN_EFI_SECURE_BOOT | not set | not set | not set | not set |

Other facts:
- **Boot parameter:** there is no `binder.impl=` style parameter. Upstream Kconfig makes the drivers mutually exclusive: `ANDROID_BINDER_IPC_RUST depends on ... !ANDROID_BINDER_IPC`, and both are `bool`. The only binder parameters are `binder.devices` and `binder.debug_mask` for the C driver, and `rust_binder.rust_devices` for the Rust one (from `modules.builtin.modinfo`).
- **cgroups:** the host is cgroup v2 only (`cgroup2fs`; controllers cpuset, cpu, io, memory, hugetlb, pids, rdma, misc, dmem). The v2 freezer (`cgroup.freeze`) is core cgroup code and is what LXC 7 uses; CGROUP_FREEZER is the v1 controller.
- **Networking:** waydroid 1.6.3 depends on `nftables`, so `waydroid-net.sh` takes the nft path. That path needs the `inet lxc` filter table and the `ip lxc` masquerade (nf_tables, nft_nat, nft_masq, bridge, veth), all autoloaded modules. The iptables fallback modules exist as well.
- **Repositories and packages:**
  - The ISO's repos are `[cachyos]`, `[core]` and `[extra]`. `pacman -Ss binder` finds `binder_linux-dkms` only in chaotic-aur, which the ISO does not configure.
  - `[cachyos]` has `linux-cachyos-lts` 6.18.60-1 with headers, `linux-cachyos-nvidia-open` 7.3.1-1 (driver 615.78.08), and `linux-cachyos-lts-nvidia-open` 6.18.60-1.
  - It also has `waydroid-image` 20.0.20260510, a 709 MB LineageOS image package.

### The 7.4 cliff

- **Upstream change:** commit `fd2bc059ed46` ("binder: rm -f binder.c", Carlos Llamas, merged through char-misc on 2026-09-17) is in v7.4-rc1, tagged 2026-11-01, but not in v7.3. On master, `ANDROID_BINDER_IPC_RUST` is the only binder and `depends on RUST`.
- **Why linux-cachyos cannot follow:** `config RUST` has `depends on !DEBUG_INFO_BTF || (PAHOLE_HAS_LANG_EXCLUDE && !LTO)`, unchanged at v7.4-rc1. linux-cachyos is built with `LTO_CLANG_THIN=y` and `DEBUG_INFO_BTF=y`, which is why its built config has no `CONFIG_RUST` even though the source config asks for it.
- **Consequence:** a 7.4 linux-cachyos built the same way has **no binder**.
- **Timing:** CachyOS usually ships a new mainline within days of release. 7.4 should be out around late December 2026 or early January 2027.

## Evidence: Waydroid on each binder variant

Source: `/usr/lib/waydroid/tools/helpers/drivers.py`, `actions/container_manager.py` and `helpers/lxc.py` (waydroid 1.6.3).

### What happens on linux-cachyos (C binder built in, binderfs, default devices)

1. **Before Waydroid runs:** at boot no `/dev/binder*` exists. With BINDERFS=y the C driver creates no misc devices (v7.3 `binder.c:7135`), and `/proc/filesystems` lists `nodev binder`.
2. **Probe:** `probeBinderDriver` finds none of `/dev/{anbox-binder,puddlejumper,bonder,binder}` or the vnd and hw variants, so it plans `anbox-binder`, `anbox-vndbinder` and `anbox-hwbinder`.
3. **No module load:** `isBinderfsLoaded()` is true, so `modprobe binder_linux` never runs. A DKMS `binder_linux` is never used. If it were loaded by hand, `register_filesystem("binder")` would fail because the name is already taken.
4. **Mount:** `mkdir -p /dev/binderfs` and `mount -t binder binder /dev/binderfs`. Every binderfs mount is a new instance, and the kernel pre-populates it from `binder.devices` (v7.3 `binderfs.c:675`): `binder-control`, `binder`, `hwbinder`, `vndbinder` and `features/`.
5. **Allocation:** `BINDER_CTL_ADD` (`0xC1086201`) adds the three `anbox-*` nodes.
6. **Symlinks:** `ln -s /dev/binderfs/* /dev/` symlinks every entry into `/dev`, including `binder-control` and `features`.
7. **Node selection at `waydroid init`:** `setupBinderNodes` loops without a `break`, so the last name that exists wins. That means `binder`, `vndbinder` and `hwbinder`, the kernel's own nodes, rather than `anbox-*`. `waydroid.cfg` and `config_nodes` (`/dev/binder → dev/binder`, `bind,create=file,optional`) freeze those names until the next `init` or `upgrade`. On zen the same code produces `anbox-*` (dev machine: `waydroid.cfg` has `binder = anbox-binder`).
8. **ashmem:** `probeAshmemDriver` fails quietly (no ashmem), and `sys.use_memfd=true` goes into `waydroid_base.prop`.

### The stale-symlink bug, precisely

- **What Waydroid does:** the links are created with `ln -s` (no `-f`) and never removed, and binderfs is never unmounted on stop.
- **Reboot is not the trigger:** `/dev` is devtmpfs, so the links do not survive a reboot. The explanation in #2422's workaround comment is wrong on that point.
- **Unmount during uptime self-heals:** if `/dev/binderfs` is unmounted while the system is up, `os.path.exists()` returns false for the dangling links. Waydroid then mounts a new instance and allocates again, and `ln -s` fails with EEXIST, silently (`check=False`). The old links point at the same `/dev/binderfs/<name>` paths, so they resolve again.
- **The real failure is crossing kernels:** initialize on linux-cachyos (config says `binder`), then boot a kernel with `BINDER_DEVICES=""` (zen, Arch `linux`). The probe allocates only `anbox-*`, so `/dev/binder` never appears. Because the mount entry is `optional`, LXC starts without it and Android waits forever for servicemanager. The dev machine boots linux, linux-zen and linux-cachyos against one `/var/lib/waydroid`.
- **Smaller issues:** `binder-control` and `features` get symlinked into `/dev`, and each remount creates a separate binder context.

### #2422 and the Rust binder fixes

- **Status:** #2422 is open. The last activity is from 2026-10-13, and nobody from the project has replied.
- **Suggested fix is irrelevant:** `8e28c67` ("correctly handle FDA objects of length zero") has been in mainline since 6.19 (`8f589c9c3be5`). It is already in every 7.2 kernel and fixes a different error.
- **Scope:** the same ENOSPC is reported on linux-omarchy 7.2.5 (omacom/omarchy#12454).
- **Upstream activity:** `drivers/android/binder` commits from 2026-07 to 7.4-rc1 add tracepoints, logs, the TF_DEFER_COMPLETE flag and refcount fixes. None of them targets allocation of transactions of about 1 MiB or more.
- **Related fix:** "rust_binder: fix oneway spam detection" (`4fc87c24`, stable) addresses async-space accounting only.
- **Conclusion:** the root cause is still unknown. linux-cachyos is not affected today because it uses the C driver.

## Evidence: SW_SYNC and the stock HWC

Source: `hwcomposer.cpp` in android_hardware_waydroid lineage-20 (`a575cfc`), and libsync `sync.c` from AOSP android13-release.

- **No HWC change is needed:** the ISO kernel does not need SW_SYNC. Its Kconfig marks it as for testing and debugging only, and no candidate kernel enables it.
- **What the stock HWC does without it:**
  - **Timeline:** `hwc_device_open` calls `sw_sync_timeline_create()`, which tries `/sys/kernel/debug/sync/sw_sync` and then `/dev/sw_sync` and returns `-1`. The HWC never checks the result.
  - **Retire fence:** each `hwc_set` calls `sw_sync_timeline_inc(-1, 1)` and `sw_sync_fence_create(-1, …)`, both failing with EBADF, so `retireFenceFd = -1`.
  - **Release fence:** every layer's `releaseFenceFd` is `-1` anyway (`// TODO: Implement per-hwc_layer explicit synchronization`).
  - **Acquire fence:** the HWC CPU-waits on it with `sync_wait(…, 100 ms)` before `wl_surface_commit`.
  - `wayland-hwc.cpp` includes `sw_sync.h` but calls nothing from it.
- **Effect in SurfaceFlinger:** through hwc2on1adapter, SurfaceFlinger receives NO_FENCE for present and release.
  - Frames count as presented at once, so FrameTimeline gets no present times and there is no present-fence back-pressure.
  - Buffers go back to producers without waiting for the host compositor; there is roughly one frame of margin with triple buffering.
  - This is how Waydroid already runs on Arch and zen hosts.
- **How Waydroid handles the paths:** `generate_nodes_lxc_config` adds `/dev/sw_sync` only if it exists (`check=True`), and always rbinds host `/sys/kernel/debug` (optional). `set_permissions` chmods `/dev/sw_sync` and `/sys/kernel/debug/sync/sw_sync` to 777 only if they exist. Separate finding: Android root can see the host's debugfs.

### Fork plan to replace sw_sync (L3)

1. **Acquire:** import SurfaceFlinger's acquire sync_file into a timeline point (`DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE` with `IMPORT_SYNC_FILE`, then `SYNCOBJ_TRANSFER`). Hand it to the compositor with `wp_linux_drm_syncobj_surface_v1.set_acquire_point`. This removes the CPU `sync_wait`, which can block `hwc_set` for up to 100 ms per layer.
2. **Why the syncobj release point cannot be exported directly:** HWC1 must return release and present fences inside `set()`, and they signal later. A timeline point the compositor has not yet attached a fence to cannot become a sync_file:
   - `HANDLE_TO_FD` with `EXPORT_SYNC_FILE` fails on a point that has no fence attached yet.
   - `TRANSFER` with `WAIT_FOR_SUBMIT` blocks until the compositor attaches one.
3. **vgem fences instead:** `DRM_VGEM` is a mainline module, `m` in every candidate kernel. In `set()` the HWC:
   1. Attaches a vgem fence (`DRM_IOCTL_VGEM_FENCE_ATTACH`) to a one-page dma-heap buffer imported into vgem with `PRIME_FD_TO_HANDLE`.
   2. Turns it into a sync_file with `DMA_BUF_IOCTL_EXPORT_SYNC_FILE` and returns that as the release or retire fence.
   3. A fence thread waits on the compositor's release point (`DRM_IOCTL_SYNCOBJ_EVENTFD`) or on `wp_presentation` presented or discarded, then calls `DRM_IOCTL_VGEM_FENCE_SIGNAL`.

   vgem signals by itself after 10 s, so a crashed compositor cannot deadlock SurfaceFlinger. The kernel UAPI for all of these is in the 7.3.1 headers.
4. **Platform work:**
   - Load `vgem` through modules-load.d.
   - Have `crownos-android-setup` add an `lxc.mount.entry` for `/dev/dri/by-path/platform-vgem-render` to `config_nodes` after every `init` or `upgrade`.
   - Always set `drm_device`. Otherwise `gpu.py` can pick vgem's render node, and on NVIDIA-only machines it will, because `nvidia` is in its `unsupported` list.
   - S2/L3 must settle which vgem node (render or card) the ioctls need inside the container.
5. **Fallback:** if vgem is missing, use the stock `-1` fences, which are known to work.

## Measurement kit (K3)

All files are in `spikes/s3-platform/`. Each script prints one JSON object.

| script | runs as | measures |
|---|---|---|
| `measure/run-baseline.sh [--no-root] [--app PKG]` | user, calls `sudo root-steps.sh` | Runs everything below in order and prints one merged JSON object. Results go to `~/.cache/crownos-android/s3/results/<time>/`. |
| `measure/boot.py` | user | Session start, payload cgroup created, "Android with user 0 is ready" (read from the session log, 50 ms polling), `sys.boot_completed` (about 1 s resolution), MemAvailable, kernel binder errors from the journal. |
| `measure/boot_events.py` (`root-steps.sh boot-events T0 T0B`) | root | `boot_progress_*`, `sf_stop_bootanim` and `wm_boot_animation_done` (uptime ms, which is host CLOCK_MONOTONIC), plus `ro.boottime.*` (CLOCK_BOOTTIME). Both are reported as offsets from boot.py's t0. |
| `measure/memory.py` | user / root | User: payload and monitor cgroup `memory.current`, `memory.peak`, `memory.swap.current`, `memory.zswap.current` and full `memory.stat`, plus the session manager's `smaps_rollup` PSS. Root adds the container manager, lxc-start and dnsmasq PSS, and the Android PSS sum with the top 15 processes. |
| `measure/android_meminfo.py` (`root-steps.sh meminfo`) | root | `dumpsys meminfo`: top entries by process, by OOM adjustment and by category; total, free, used and lost RAM. |
| `measure/idle.py` | user / root | Payload and monitor CPU % of one core from `cpu.stat`, host Waydroid process CPU, context switches per second as a wakeup proxy, and the frozen state. Root adds exact Android wakeups per second: `perf record sched:sched_wakeup` filtered by wakee TID, streamed. |
| `measure/lifecycle.py` | user | Freeze and Unfreeze over D-Bus (call time and the time until `cgroup.events` frozen changes), CPU while frozen, session stop until the cgroup and lxc-start are gone and the rootfs is unmounted, leftovers, a warm start and stop, and MemAvailable before start, while running, while frozen and after stop. |
| `measure/binder_check.py` (`root-steps.sh binder-check`) | user / root | Binder driver kind, binderfs registration and mounts, node symlinks (including dangling ones), `waydroid.cfg` and `config_nodes` names. Root adds dmesg binder failures and ENOSPC counts. |
| `root-steps.sh service-start/service-stop/drop-caches/stay-awake/idle/memory/meminfo` | root | Timed `waydroid-container.service` start and stop, a cold page cache, and `svc power stayon true`, which stops Waydroid auto-freezing Android during idle sampling. |
| `root-steps.sh binder-nodes` | root, optional, changes the system | Prototype of the setup binder step (see the recommendations). |
| `kernel-check.sh CONFIG...` | user | The option table above. Accepts `.config`, `config.gz` or a vmlinuz. |
| `qemu-snapshot.sh build IMAGES_DIR [ISO] / overlay NAME / run NAME` | user | Reusable functional-test VM; see below. |

**Payload cgroup:** the scripts resolve it at runtime from lxc-start's child and report the path. A privileged LXC started by root normally uses `/sys/fs/cgroup/lxc.payload.waydroid`, with the monitor in `lxc.monitor.waydroid`.

**D-Bus policy:** Waydroid's policy opens `id.waydro.Container` Stop, Freeze and Unfreeze to every local user, so those timings need no root. Only `waydroid shell`, `waydroid logcat`, `smaps_rollup` of root-owned processes, `perf`, `systemctl` and `drop_caches` need root.

**Smoke test (Android stopped, zen):**
- `binder_check.py` reports Rust binder, binderfs registered and unmounted, and `anbox-*` names in the config.
- `memory.py` and `idle.py` handle a missing container.
- The meminfo parser was checked against right-aligned sample output.

### QEMU snapshot (`qemu-snapshot.sh`, not run)

The machine is the same as `install-test.sh`: q35, OVMF 4m, a virtio disk, and `-fw_cfg opt/org.crownos/autoinstall`.

1. **Install:** an offline unattended install to a 32 GiB raw disk, converted to `base.qcow2` under `~/.cache/crownos-android/s3/qemu`.
2. **Provisioning hook:** the autoinstall `kernel_args` carry a one-shot `systemd.run=` that runs `provision.sh`, handed in through `fw_cfg opt/org.crownos.s3/provision`. The autoinstall schema has no Android option or post-install hook yet, because L8 adds those.
3. **Images:** the user's `system.img` and `vendor.img` are attached read-only as virtio disks with the serials `wd-system` and `wd-vendor`.
4. **Provisioning:** `provision.sh`:
   1. Copies the images into the guest.
   2. Runs `pacman -S waydroid` over user-mode networking, if the ISO does not already include it.
   3. Runs `waydroid init -f -i` and enables `waydroid-container.service`.
   4. Removes the hook from `/efi/loader/entries/*.conf` and `/etc/kernel/cmdline`, then powers off.
5. **Reuse:** the base and its OVMF vars are made read-only, and tests boot qcow2 overlays (`overlay NAME`, `run NAME`, with SSH on 127.0.0.1:2222 and QMP).

**GPU and gpu.py:**
- `virtio_gpu` is not in `gpu.py`'s `unsupported` list, so the virtio render node becomes `gralloc=gbm`, `egl=mesa`, `gralloc.gbm.device=/dev/dri/renderD128`.
- `virtio_gpu` is not in the `getVulkanDriver` table, so `ro.hardware.vulkan` stays unset. There is no Vulkan HAL and no Venus.
- `GPU_MODE=virgl` (virtio-vga-gl, egl-headless) uses that path. It is NEEDS-RUNTIME that the LOS20 vendor Mesa includes virgl.
- `GPU_MODE=soft` (the default) uses plain virtio-vga and forces SwiftShader through `waydroid.cfg [properties]`. Those properties win over the generated base props in `make_base_props`.
- QEMU is for functional tests only; the budgets come from real hardware.

## zram and swap

| | dev machine | ISO and installed system |
|---|---|---|
| compressed swap | zram0 8 GiB zstd (prio 100), zswap disabled on the cmdline | none configured; linux-cachyos turns on **zswap** by default (zstd, shrinker on) |
| backing swap | `/swapfile` 8 GiB (prio 10) | swap partition the size of RAM, capped at 16 GiB (installer `MAX_SWAP_BYTES`) |
| reclaim | MGLRU on, swappiness 60, page-cluster 0 | MGLRU on, kernel defaults (swappiness 60, page-cluster 3) |
| state seen | zram full (6.7 G of data, 2 G compressed) and the swapfile 7.9 G used: an LRU inversion, where older cold pages stay in zram while newer ones go to disk | – |

**Recommendation for 8 GB laptops with a 1–1.5 GB Android:** keep the ISO's zswap in front of the RAM-sized partition and do not add zram. zswap writes its coldest entries back in LRU order, which avoids the inversion above, and the partition still allows hibernation. Add a sysctl and tmpfiles drop-in in crownos-iso:
- `zswap.max_pool_percent=25`
- `vm.swappiness=100`, so compressed swap can absorb idle ART and native heaps before desktop file cache is dropped
- `vm.page-cluster=0`, because Android's swap-in pattern is random
- `/sys/kernel/mm/lru_gen/min_ttl_ms=1000`, MGLRU's documented guard against thrashing

Only on installs without a swap partition, use zram-generator with `zram-size = min(ram / 2, 4096)`, `compression-algorithm = zstd`, `swap-priority = 100`.

Android's lmkd sees host-wide `/proc/pressure/memory`. The payload cgroup sits outside systemd units, so systemd-oomd and `MemoryHigh` cannot act on it today; the idle stop and lmkd are what limit it. Verify all of this with `memory.zswap.current`, `Zswap`/`Zswapped` and PSI from `memory.py`.

## Recommendations

1. **Keep linux-cachyos with the built-in C binder for the 7.2 and 7.3 ISOs. Never ship binder DKMS.**
   - DKMS needs a compiler on users' machines and breaks on new kernels.
   - Under Secure Boot it needs MOK enrollment as soon as CrownOS enforces module signatures (`module.sig_enforce` or `lockdown`). Today `MODULE_SIG_FORCE` and `LOCK_DOWN_IN_EFI_SECURE_BOOT` are off, so unsigned modules would load with only a taint, which is no security posture.
   - A built-in driver needs nothing beyond the signed kernel or UKI.
   - Precompiled `linux-cachyos-*-nvidia-open` modules are built in the same PKGBUILD run as the kernel (`_build_nvidia_open`), so they can carry its build-key signature. Verify this before relying on it.
2. **Deterministic binder nodes on every kernel.** Before `waydroid init`, and as an `ExecStartPre=` drop-in for `waydroid-container.service`, `crownos-android-setup` should:
   1. Mount binderfs if it is not mounted.
   2. Make sure `binder`, `vndbinder` and `hwbinder` exist, using `BINDER_CTL_ADD` when the kernel did not pre-create them.
   3. Run `ln -sfn` for each into `/dev`.
   4. Pin `waydroid.cfg` to those names.

   Waydroid then finds `/dev/binder` and allocates nothing. This removes the `anbox-*` versus `binder` split, the dangling-link case and the cross-kernel failure. The prototype is `root-steps.sh binder-nodes`. The probe should write the driver kind (`/sys/module/binder` or `rust_binder`) to `setup.json`.
3. **Decide the post-7.3 kernel by 2026-12-15, before linux-cachyos moves to 7.4.**
   - **(a)** Android-enabled installs, or the whole ISO, use `linux-cachyos-lts` 6.18. It has the C binder, EOL December 2028, and an LTS nvidia-open package. The ESP must fit a second kernel when it is shared with Windows (300 MiB is "comfortable").
   - **(b)** CrownOS builds linux-cachyos without LTO, or without BTF, so Rust and the Rust binder are enabled. Only once the large-transaction test passes.
   - **(c)** Follow CachyOS, if it reacts.

   Until then, raise this with CachyOS (with 7.4 its default kernel loses binder) and keep a reproducer for #2422 in the bench.
4. **Measure on linux-cachyos, not zen.** The dev machine already has `linux-cachyos 7.2.9-2`. Run `measure/run-baseline.sh --app <pkg>` there.
5. **Implement the K2 fork plan in L3:** syncobj acquire, vgem release and present fences, and the stock `-1` fallback. Fix plan risk 3's wording to match.

## Risks

- **7.4 removes the C binder and linux-cachyos cannot build Rust:** without recommendation 3, the first 7.4 update leaves Android with no binder on every CrownOS install.
- **Rust binder ENOSPC (#2422) is not root-caused:** any option that ends up on the Rust binder needs a large-transaction test first.
- **Waydroid's D-Bus policy lets any local user stop, freeze or unfreeze the container,** and `Freeze` busy-loops `lxc-info`. The plan's "used by another user" guard must not rely on Waydroid alone.
- **Host debugfs is rbound into the container,** so Android root can see it. `set_permissions` makes every `/dev/dri/renderD*` and `/dev/dma_heap/*` mode 777 host-wide.
- **The vgem fence path is unproven inside the container** (render node or card node, and dma-heap import); the fallback is the stock `-1` fences. Loading vgem without pinning `drm_device` can make `gpu.py` select vgem.
- **Measurement noise:** the dev machine has 15 GiB of RAM with about 15 GiB of swap in use and is shared with other agents, so MemAvailable deltas are noisy. The scripts record PSI and meminfo next to every sample, and the budget numbers must come from the hardware matrix.
- **Unconfirmed until runtime:**
  - the lxc cgroup path
  - that the C binder on linux-cachyos boots LOS20 cleanly
  - that `svc power stayon` prevents Waydroid's suspend freeze
  - that the vendor Mesa has virgl
