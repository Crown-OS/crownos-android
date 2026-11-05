# Lane `venus`: NVIDIA-only GPU tier

## Repos and owned paths
- **crownos-android**, worktree `~/crownos-lanes/venus/crownos-android`, branch `android/venus`. You own `crates/venus/` (keep the contract 13 signatures exactly) and `packaging/venus/`.
- **crownos-pkgbuilds**, worktree `~/crownos-lanes/venus/crownos-pkgbuilds`, branch `android/venus`. You own `crownos-android-venus/`.

Read `docs/decisions/s4-venus.md` and everything under `spikes/s4-venus/` first. The spike's builds and artifacts are in `~/.cache/crownos-android/s4/`.

## Goal
Make the S4 chain installable and maintainable as the `crownos-android-venus` package, with automatic detection.

## Requirements
1. **`crates/venus` bodies.**
   - `detect(sys, proc_root)` implements the S4 detection: the boot_vga card's driver is `nvidia`; `proc_root/driver/nvidia/version` says "Open Kernel Module"; driver major ≥ `MIN_DRIVER_MAJOR`; `sys/module/nvidia_drm/parameters/modeset` is `Y`; `/dev/udmabuf` exists (take it as a parameter or derive it from the roots, so tests use fake trees). It returns the render node.
   - `overlay_files`, `props` and `container_mount_line` stay as they are, unless S4 shows something missing (for example the ANGLE files). Changing their output is allowed; changing their signatures is not.
   - Tests over fake sysfs/procfs trees: hybrid AMD+NVIDIA means None, closed module means None, an old driver means None, and the happy path means Some.
2. **`packaging/venus/`.**
   - `build.sh` reproduces the S4 builds from pinned sources: the patched Mesa venus driver for x86_64 and x86, the vtest gralloc backend, and the patched host virglrenderer.
   - Patches live as files under `packaging/venus/patches/`, and sources are pinned by commit and sha256.
   - The output tree is `overlay/` (the files `overlay_files()` names) plus `host/`: `virgl_test_server`, `virgl_render_server` and `libvirglrenderer.so.1`.
   - Rebasing onto a stable Mesa is a follow-up; record which Mesa version you built and why.
3. **User unit `crownos-android-render.service`**, for `venus::RENDER_SERVICE`:
   - runs the host server with the S4 arguments on `%h/.local/share/waydroid/data/crownos/venus/venus.sock`;
   - `ExecStartPre` creates the socket dir if missing; the setup service creates it as root when ownership needs it, so coordinate through the `prepare-container` step and say what it must do in your report;
   - `Restart=on-failure`;
   - no `[Install]`, because the daemon starts it.

   Also ship the udev rule giving `uaccess` on `/dev/udmabuf`.
4. **`crownos-pkgbuilds/crownos-android-venus/PKGBUILD`:**
   - builds `packaging/venus/build.sh` output from source, or packages a prebuilt artifact tarball with sha256;
   - installs the overlay files to `venus::OVERLAY_SOURCE`, the host binaries to `/usr/lib/crownos-android/venus/`, the unit and the udev rule;
   - depends on `crownos-android` and `nvidia-utils`.
5. **NVIDIA driver matrix:** document it in `packaging/venus/README.md`: tested driver branch, how to verify a new branch, and known gaps (no hardware video decode, ETC2, FP16).

## Acceptance
- **Tests:** crate tests green, and `build.sh` reproduces the artifacts. If the full build exceeds the machine's limits, run it under build-slot with `-j4`; the S4 builds took seconds.
- **Packaging:** `makepkg --printsrcinfo` works, and the unit passes `systemd-analyze verify --user` where possible.
- **Commits:** on `android/venus` in both worktrees.
