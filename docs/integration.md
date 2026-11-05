# Integration follow-ups

Collected from the lane reports. The lead handles these when merging the lane branches, unless a line says otherwise.

## Lead, at merge
- **Setup unit type.** Change `crownos-android-setup.service` to `Type=exec` (iso lane). A oneshot keeps a start job open for the whole download, so the boot never finishes and tty1 shows "A start job is running".
- **Settings re-enable.** Switch Settings' `SET_UP_AGAIN` (`crownos-settings/src/net/android/removal.rs`) to `pkexec /usr/lib/crownos-android/crownos-android-setup enable`, which is contract 12's new command.
- **Autoinstall key.** Check that `crownos-iso/tests/autoinstall-android.json` uses the installer's key: `"android_apps": {"enabled": {"google_play": true}}`.
- **pkexec path.** Check that the pkexec action's `exec.path` is `/usr/lib/crownos-android/crownos-android-setup`.
- **Lockfiles.** Refresh Cargo.lock files after the merges. Review the installer's lock refresh commit `2991783`, which may reflect the user's uncommitted crownui working copy.
- **Licenses.** Add LICENSE files to crownos-android and install them in the PKGBUILD.
- **vgem.** Decide on vgem release fences with the hwc lane's result. If they are used, ship `modules-load.d/crownos-android.conf` with `vgem`, bind its node into the container, and always set `drm_device`.

## User, when merging into their own repos
- **crownos-config.** The `android/base` branch, carrying the android section and `android.ron`, must be merged with the uncommitted clock and lock-transition work in `/e/CrownOS/crownos-config`.
  - Settings and crownpositor build against both.
- **Settings in the lane layout.** Settings only builds in the lane layout through an overlay, because its `crownpositor` dependency points at `/e`'s uncommitted lockscreen code.
- **Windows migration assistant.** `windows-migration-assistant/src/oobe.rs` needs `android_apps`, and it is already missing `ai_features`. That repo has no commits, so no lane could touch it.
- **Rust toolchain.** The rustup `stable` toolchain is half-installed ("missing manifest"). Lanes pinned `1.97-x86_64-unknown-linux-gnu`. Repair it with `rustup toolchain install stable --force` when convenient.

## Release
- **Artifacts.** Publish crownos-android and crownos-android-venus, plus `bundle-<version>.tar.zst` with `manifest.json` and `manifest.json.minisig` at its root, from the crownos-android release. Then update the PKGBUILD commit pins.
- **Signing key.** Generate the release minisign key, replacing the dev key in `setup/src/keys.rs` (setup lane).
- **CI.** Decide whether CI runs `install-test.sh --android`, which needs e2fsprogs.
- **ISO size.** `crownos-android-venus` pulls `nvidia-utils` into the offline mirror, which makes the ISO bigger.

## Behaviour to confirm with the user
- **Loader under Do Not Disturb.** DND decides only where a resident notification first lands. A loader the user swiped away keeps updating in the notification center instead of popping up again.
- **Start control.** Settings has no "Start Android" control, because contract 4 has no start-without-launch call.
- **Daemon lifetime.** Settings keeps the daemon alive while its Android page has been opened and the window is still open.
