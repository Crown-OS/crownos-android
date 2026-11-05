# Lane `installer`: installer option and Settings page

## Repos and owned paths
- **crownos-installer**, worktree `~/crownos-lanes/installer/crownos-installer`, branch `android/installer`.
- **crownos-settings**, worktree `~/crownos-lanes/installer/crownos-settings`, branch `android/installer`.

The crownos-config in your lane dir is the `android/base` branch, which already has `schema::Android` and `system::AndroidConfig`. Read it, don't edit it. crownos-android is a symlink to the lead's checkout: read `crates/android-protocol` from it, and depend on it by path `../crownos-android/crates/android-protocol` where needed.

## Goal
1. An optional "Android apps" switch at install time.
2. A Settings page that manages Android apps afterwards.

## Installer requirements
Follow the `AiFeatures` flow end to end (commit `bf18d3f`; `docs/decisions` has nothing on it, so read the code):
- `oobe.rs`
- `ui/wizard.rs`
- `ui/pages/confirm.rs::ai_features`
- `engine/plan.rs::system_steps`
- `engine/exec.rs::commands_for`
- `autoinstall.rs`
- the tests

### 1. Wizard state
- **Type:** in `oobe.rs` add `AndroidApps { Disabled (default), Enabled { google_play: bool } }`, serde snake_case, with `#[serde(default, skip_serializing_if = "AndroidApps::is_disabled")]` on `Install.android_apps`.
- **Wizard:** a `Wizard.android_apps` field, `set_android_apps()` and a restaged plan.
- **Autoinstall:** `android_apps` in `autoinstall::Config`, with `#[serde(default)]`.

### 2. Confirm page
- **Rows:** a switch row "Android apps", captioned "Runs Android apps in their own windows. About 1 GB downloads in the background after the first start." When it is on, a second switch "Include Google Play" (default on), captioned "Needs a one-time device registration. Some banking apps don't work."
- **Visibility:** hide both when the machine can't run Android: kernel page size isn't 4 KiB (read it in the probe), or the architecture isn't x86_64 or aarch64.

### 3. Plan
- **Packages:** with Enabled, add `waydroid lxc python-gbinder libgbinder libglibutil dnsmasq crownos-android` to the pacstrap set. Add `crownos-android-venus` when the probe found an NVIDIA Turing+ GPU (`util/pci.rs::is_nvidia_turing_or_later`) and no other GPU.
- **Step:** add `Step::ConfigureAndroid { root, google_play }` with the label "Setting up Android apps". It runs:
  1. `Cmd::WriteFile` `/etc/crownos/android.ron`. Generate the text with `AndroidConfig { enabled: true, google_play, tier: None, bundle: None }.to_ron()`, or write the equivalent RON literal if the installer must not depend on crownos-config.
  2. `MakeDirs` `/var/lib/crownos/android`, then touch the `pending` marker.
  3. `systemctl enable crownos-android-setup.service` in the chroot.
  4. `systemctl --global enable crown-androidd.socket`.
- **Windows migration assistant:** it has a copy of `oobe.rs`, but its repo has no commits, so leave it alone and mention it in your report.

### 4. Tests
- A plan test like `ai_features_install_the_detected_stack_and_only_when_asked`, for Android.
- A window test that clicks the switch and sees the step appear.
- Update every `Install { .. }` fixture.
- Check `--print-plan`.

## Settings requirements
Add a page "Android apps", slug `android`, following `pages/voice.rs` and `net/voice/`:
- **Status:** NotInstalled, setting up (with progress), Stopped, Running (window count), SafeMode, Unsupported (with reason). Get it from `android_protocol::android` status plus `RuntimeChanged`/`SetupProgress` events, through `AsyncClient` in a `net/android/` worker. Distinguish NotRunning from NotInstalled, as the voice page does.
- **Controls:**
  - Start Android: launch nothing, just start the runtime. If there is no IPC call for that, hide the control.
  - Stop Android now: `stop(User)`.
  - "Stop when unused": a choice of 1, 5, 15 or 30 minutes, or Never, writing `crownos_config::Android.idle_minutes`.
  - "Share Downloads" and "Share Pictures" switches.
- **Google Play:** when `google_play` is set, a "Register this device" row that shows `google_registration_id()` with a copy button and opens `https://www.google.com/android/uncertified`.
- **About:** GPU tier from `AndroidConfig.tier` (Native, Venus or Software with a short explanation), and bundle version.
- **Storage:** the size of `/var/lib/waydroid/images` (readable) and of the user's data dir (best effort).
- **Danger zone:**
  - "Reset Android data" runs `pkexec crownos-android-setup remove --wipe-data`, then re-enables setup.
  - "Remove Android apps" runs `pkexec crownos-android-setup remove`.
  - Both need a confirmation dialog using the existing settings controls.
- **Registration:** register the page in `pages/mod.rs` `GROUPS` and add the `android` section to `src/config.rs` `sections!`.

## Acceptance
- **Installer:** gates green, including the new plan and window tests.
- **Settings:** gates green, with page-logic tests for the status mapping and the idle-choice mapping.
- **Commits:** on `android/installer` in both worktrees.
