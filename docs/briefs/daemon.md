# Lane `daemon`: crown-androidd and crown-android

## Repo and owned paths
**crownos-android**, worktree `~/crownos-lanes/daemon/crownos-android`, branch `android/daemon`. You own:
- `crates/runtime/`
- `crates/desktop-entries/`
- `crates/splash/`
- `daemon/`
- `cli/`

Read, never edit: the contract crates, and `crates/venus` (call its API).

## Goal
The user-session manager. It starts Android on demand, shows a splash instantly, stops Android completely after the idle timeout, keeps desktop entries in sync, and turns setup progress into the loader notification.

## Requirements

### 1. Activation and serving
- **`crown-androidd`** is socket-activated on `%t/crownos/crownos-android.sock`, unit `crown-androidd.socket`, the crown-voiced pattern (`/e/CrownOS/crownos-voice/voiced/src/activation.rs`, `event_loop.rs`).
- **Protocol:** serve `android_protocol::android` (contract 4) with crownos-ipc.
- **Lifetime:** exit only when Android is stopped, no client is connected, and nothing is pending for 30 s.

### 2. Runtime crate
Waydroid control: a pure state machine plus thin IO adapters.
- **States:** NotInstalled, SettingUp, Stopped, Starting, Running, Stopping, SafeMode and Unsupported, from `android_protocol`.
  - NotInstalled: `/etc/crownos/android.ron` is missing or disabled. Read it through `crownos_config::system::AndroidConfig`; the crownos-config in your lane dir is the `android/base` branch.
  - SettingUp: the pending marker exists. Follow `setup.json` with inotify.
- **Start sequence:**
  1. Ensure `waydroid-container.service` is running: D-Bus systemd1 `StartUnit`, allowed by a polkit rule the iso lane ships.
  2. Create a `wp_security_context_v1` listener socket at `$XDG_RUNTIME_DIR/crownos/android-wayland.sock`, with sandbox engine `id.waydro.waydroid`, app id `waydroid` and a fresh UUID as instance id. Keep its close_fd for the whole session; the daemon's own Wayland connection lives as long as the session.
  3. Prepare the private `XDG_DATA_HOME` `~/.local/share/crownos/android/`, with `waydroid` symlinked to `~/.local/share/waydroid`.
  4. Start the user unit `crownos-android-session.service`, passing `WAYLAND_DISPLAY`=socket path and `XDG_DATA_HOME` through `systemctl --user set-environment` (or D-Bus SetEnvironment) right before StartUnit.
  5. On the Venus tier (`AndroidConfig.tier`), start `venus::RENDER_SERVICE` first.
  6. Connect to `<waydroid_data>/crownos/agent.sock` when it appears, do the `Hello` exchange, and wait for `Ready`.
- **Safe mode:**
  - **Triggers:** a bundle mismatch in Hello (compare with `AndroidConfig.bundle`), no Ready within 120 s, or 3 failed boots in a row (persisted in `~/.local/state/crownos/android.ron`).
  - **Action:** set `persist.crownos.safe_mode 1` with `waydroid prop set`, restart once, and post a notification.
- **Idle stop:**
  - **Timer:** the window count comes from the agent's TaskOpened/TaskClosed events. With no windows, start a timer of `crownos_config::Android.idle_minutes`, followed live with subscribe; 0 means never.
  - **Keep-alive:** while waiting, send `QueryKeepAlive` every 60 s. Any keep-alive reason restarts the timer.
  - **Stop sequence:**
    1. Send Shutdown to the agent.
    2. Call `id.waydro.SessionManager.Stop` (or stop the session unit).
    3. Call Container `Stop`.
    4. Stop `waydroid-container.service`, so no Waydroid process remains, and stop the render service.
  - **Shape:** pure and unit-tested with a fake clock.
- **Laptop sleep:** take a logind PrepareForSleep delay inhibitor. Before sleep call Container `Freeze`, and after resume `Unfreeze`. On the Venus tier, restart the render service after resume.
- **Other users:** when the container's `GetSession` reports another user's uid, report Unsupported with a message.

### 3. Launch
`launch(package, token)`:
1. Show the splash right away.
2. Start Android when it is stopped.
3. Send agent `Launch{package, width, height, token}`. The size is a sensible default: 60% of the primary output, clamped to 1280x800 logical times scale. Get the scale from the splash's own fractional-scale feedback.
4. Return `Opening` or `Starting`.

Repeated launches of a running app just activate it, through the app-icons protocol or by relaunching, which Android focuses.

### 4. Splash crate
A small Wayland client window:
- `xdg_toplevel` with app_id `waydroid.<pkg>` and title = app name, SSD through xdg-decoration;
- the icon (PNG from the desktop entry) centred on a background colour derived from the icon;
- `crownos_android_shell_manager_v1.mark_placeholder(toplevel, app_id)`.

It closes when the compositor sends `close` (the handoff), when the launch fails (show the reason for 3 s), or after 120 s. Draw with wl_shm and tiny-skia at the preferred fractional scale. It must appear within 100 ms of the launch call: keep the daemon's Wayland connection open and decode icons lazily.

### 5. Desktop entries crate
- **Mirroring:** watch `~/.local/share/crownos/android/applications/waydroid.*.desktop`, the files Waydroid's user_manager writes. Mirror each into `~/.local/share/applications/` with:
  - `Exec=crown-android launch <pkg>`;
  - `StartupWMClass=waydroid.<pkg>`;
  - Name, Icon, Categories and NoDisplay kept;
  - the app-settings action rewritten to `crown-android settings <pkg>`.
- **Removal:** delete a mirror when its source goes.
- **Handlers:** also install `crown-android-handler.desktop` handling `application/vnd.android.package-archive` (Exec `crown-android install %f`) and `x-scheme-handler/market` (`crown-android open %u`).
- **Shape:** pure rewrite functions with unit tests.

### 6. Setup progress and notifications
Follow `setup.json` and drive one resident crownotify notification:
- **Loader:** spinner when `percent` is None, otherwise the percentage, with the stage message.
- **Success:** on Ready, run the first warm boot (start, wait Ready, stop) showing "Finishing setup", then "Android apps are ready".
  - With Google Play, add the action "Register this device", which opens the uncertified-device page and copies the GSF id.
  - Add "Open Play Store" or "Open Settings".
- **Failure:** show the failure's detail, and a "Try again" action when it is retryable (`systemctl start crownos-android-setup.service`).
- **Transport:** use crownotify's crownos-ipc method `send_progress_notification` (contract 7). crownotify's own proto does not have it yet in your lane, so declare a client-only `crownos_ipc::protocol!` module for service `"crownotify"` with exactly the contract 7 signature, plus `close_notification` copied from `/e/CrownOS/crownotify/src/proto.rs`.
- **Fallback:** org.freedesktop.Notifications with the `value` hint when crownotify is not reachable.
- **Events:** emit `SetupProgress` and `RuntimeChanged` IPC events.

### 7. CLI
`crown-android` provides: `launch <pkg>` (passing `XDG_ACTIVATION_TOKEN`), `stop`, `status`, `diagnose`, `install <apk>`, `open <uri>`, `settings <pkg>`.
- **install:** copies the APK into `<data>/crownos/inbox/` (create it with mode 0700 if missing), then asks the daemon.
- **diagnose:** writes `~/.cache/crownos-android/diagnose-<time>.tar.zst` with daemon state, unit journals, `/var/lib/waydroid/waydroid.log` and the bundle/image versions.

## Acceptance
- **Unit tests:** the runtime state machine (start, idle stop, keep-alive, safe-mode triggers, sleep), the desktop entry rewrite, splash layout and colour, and the launch-size rule.
- **Daemon-to-agent tests:** a fake agent over a UnixStream pair speaking agent-wire.
- **IPC tests:** an in-process crownos-ipc round trip of every method.
- **Gates:** green. **Commits:** on `android/daemon`.
