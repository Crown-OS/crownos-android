# Lane `setup`: crownos-android-setup and the overlay bundle

## Repo and owned paths
**crownos-android**, worktree `~/crownos-lanes/setup/crownos-android`, branch `android/setup`. You own `setup/` (the root binary) and `overlay/` (the bundle build tooling). Use the contract crates read-only, and call `crates/venus` without editing it.

## Goal
After first boot, turn an installed system into a ready Android runtime in the background, safely and resumably. Also produce the signed overlay bundle that makes Android windows native.

## Requirements

### 1. `crownos-android-setup run`, root oneshot (contract 12)
1. **Read** `/etc/crownos/android.ron` through `crownos_config::system::AndroidConfig` (the `android/base` crownos-config is in your lane dir). Exit 0 when it is disabled or the pending marker is missing.
2. **Probe** (pure functions over a sysfs/procfs root, so tests use fake trees):
   - binderfs in `/proc/filesystems`;
   - page size (16 KiB means Unsupported);
   - at least 6 GiB free on `/var/lib`;
   - the GPU tier: the compositor GPU is the `boot_vga` card's render node, the same rule as crownpositor's `udev::primary_gpu`. Native when its driver is amdgpu, i915, xe, msm, panfrost, panthor, asahi or nouveau. Venus when `venus::detect` returns Some. Otherwise Software, rendering on any non-NVIDIA render node if one exists, else SwiftShader;
   - firewall conflicts: firewalld, ufw or docker present. Note them; the iso lane ships the fix.

   Write the tier into android.ron.
3. **Wait for the network** with backoff, reporting `WaitingForNetwork`.
4. **Images:**
   - Take the pinned images for `(google_play ? Gapps : Vanilla, arch)` from the shipped bundle at `/usr/share/crownos-android/bundle/<version>/` (`manifest.json` + `manifest.json.minisig`). Verify the manifest with the CrownOS public key in `setup/src/keys.rs`, using a placeholder dev key whose generation is documented in `overlay/README.md`.
   - Download with HTTP Range resume (ureq with rustls, or another small client) into `/var/lib/crownos/android/staging/`. Report percent over total bytes, verify sha256 against the pin, and unzip into `/var/lib/crownos/android/images/<system>-<vendor>/`.
   - Resume works after a reboot.
5. **`waydroid init -f -i <images dir>`.** Then point `system_ota`/`vendor_ota` in `waydroid.cfg` at `file:///var/lib/crownos/android/channel/{system,vendor}.json`, written from `Manifest::ota_channel`, so Waydroid's updater only ever sees pinned images. Write `drm_device` for the chosen render node.
6. **Bundle install:**
   - Clear `overlay_rw` and `overlay_work`.
   - Copy each manifest file (validated `OverlayPath`) into `/var/lib/waydroid/overlay/`, verifying sha256.
   - Create whiteouts as char devices 0:0 (mknod).
   - Merge the bundle props and the Venus props (on that tier) into `waydroid.cfg [properties]`. The file is read by Python configparser, so keep keys case-sensitive and the format valid.
   - Run `waydroid upgrade --offline`.
   - On the Venus tier, copy the files from `venus::OVERLAY_SOURCE`.
   - Record the bundle version in android.ron.
7. **Finish:** remove the pending marker and write Stage Ready.
   - Every stage writes `setup.json` through `setup_state::store`.
   - Failures write `SetupStatus::failed` with the right FailureCode and retryable flag.
   - Run at nice/idle priority. The unit sets it, but also lower your own IO priority.

### 2. `crownos-android-setup prepare-container`
For the `waydroid-container.service` ExecStartPre drop-in:
- mount binderfs if needed;
- create `binder`, `vndbinder` and `hwbinder` with BINDER_CTL_ADD;
- symlink them with `ln -sfn` semantics, and pin the names in waydroid.cfg (`s3-platform.md` root step `binder-nodes`);
- on the Venus tier, make sure `venus::container_mount_line` is present in `/var/lib/waydroid/lxc/waydroid/config_nodes`.

It must be idempotent and fast.

### 3. `crownos-android-setup remove [--wipe-data]`
For Settings, through pkexec:
- stop Waydroid;
- remove the images, the channel, the overlay files the bundle lists, android.ron's `enabled` flag (set it false) and the marker;
- with `--wipe-data`, also every user's `~/.local/share/waydroid`.

Never touch anything outside these paths.

### 4. `crownos-android-setup make-manifest --root <dir> --version <v> --pins <json>`
Writes `manifest.json` for an unpacked bundle tree, with sha256 values, file modes, whiteouts from `<dir>/whiteouts.txt` and the contract 11 props.

### 5. Bundle build (`overlay/`)
- **`overlay/build-bundle.sh`** assembles a bundle tree from the build artifacts:
  - the HWC .so from `~/.cache/crownos-android/hwc/out/x86_64/vendor/lib64/hw/hwcomposer.waydroid.so`;
  - the agent jar, init rc, overlay APKs and blank HOME from `~/.cache/crownos-android/agent/out/`.

  When an artifact is missing, it fails with a clear message.
- **Placement:** `vendor/lib64/hw/`, `system/framework/crown-agent.jar`, `system/etc/init/crown-agent.rc`, `system/product/overlay/*.apk` and `system/product/app/CrownHome/CrownHome.apk`.
- **Manifest:** generated with `make-manifest`, then signed with `minisign` when a key is given.
- **Pins:** `overlay/pins.json` pins the images. Get the currently installed GAPPS build (`system_datetime 1775208984`, vendor `1777409843`) and the matching VANILLA build from the Waydroid OTA JSON (`https://ota.waydro.id/system/lineage/waydroid_x86_64/{GAPPS,VANILLA}.json`, `vendor/waydroid_x86_64/MAINLINE.json`) with a helper script. Commit the resulting JSON.
- **Docs:** `overlay/README.md` covers release signing-key handling and how to rebuild for a new image pin (re-run the hwc `ndk/build.sh` against the new images).

## Acceptance
- **Probe tests:** each tier decision against fake sysfs trees.
- **Download tests:** against a local `std::net::TcpListener` HTTP server with Range, an interrupted transfer and a checksum mismatch.
- **Bundle install tests:** into a temp overlay root, including rejected paths, whiteouts (skipped when unprivileged), props merge and idempotence.
- **Other tests:** prepare-container line management, make-manifest output that `Manifest` parses, and setup.json stage transitions.
- **Gates:** green. **Commits:** on `android/setup`.
