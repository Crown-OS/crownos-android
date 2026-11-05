# Contracts

Every lane builds against these contracts and nothing else. A contract changes only through the lead, with a version bump and updated golden vectors.

| # | Contract | Status | Source of truth |
|---|---|---|---|
| 1 | `crownos-android-shell-v1` Wayland protocol | frozen v1 | `crownos-protocols` branch `android/base`, `protocols/crownos-android-shell-v1.xml` |
| 2 | HWC ⇄ agent records | frozen v1 | `crates/agent-wire` and `agent-wire-vectors.txt` |
| 3 | daemon ⇄ agent records | frozen v1 | `crates/agent-wire` and `agent-wire-vectors.txt` |
| 4 | `crownos-android` crownos-ipc service | frozen v1 (install_package, open_uri, open_app_settings appended at integration) | `crates/android-protocol` |
| 5 | `setup.json` | frozen v1 | `crates/setup-state` |
| 6 | `android.ron` (system and user) | frozen v1 | `crownos-config` branch `android/base`: `system/android.rs`, `schema/android.rs` |
| 7 | crownotify progress notifications | frozen v1 | below; the notify lane adds it to `crownotify/src/proto.rs` |
| 8 | overlay bundle manifest | frozen v1 | `crates/bundle` (`Manifest`) |
| 9 | image pins and OTA channel | frozen v1 | `crates/bundle` (`ImagePin`, `OtaChannel`) |
| 10 | naming and packaging | frozen v1 | below |
| 11 | Android properties | frozen v1 | below |
| 12 | `crownos-android-setup` command line | frozen v1 | below |
| 13 | `crates/venus` API | frozen v1 | `crates/venus/src/lib.rs` signatures |

## 11. Android properties

The bundle's `props` set these through `waydroid.cfg [properties]`. The HWC and the agent read them; nothing else may depend on other props.

| Property | Value | Read by | Meaning |
|---|---|---|---|
| `ro.crownos.bundle` | bundle version | HWC, agent | Sent in `Hello`. A mismatch with the peer means safe mode. |
| `persist.crownos.safe_mode` | `0` or `1` | HWC, agent | `1`: the agent does not organize tasks, and the HWC uses stock Waydroid multi-window. Set at runtime by the daemon through `waydroid prop set`. |
| `persist.waydroid.multi_windows` | `true` | HWC | Per-task windows. |
| `persist.waydroid.width`, `persist.waydroid.height` | `4096` | HWC | Fixed Android display size in pixels: the atlas the agent packs tasks into. Never follows a host configure. |
| `persist.wm.debug.caption_on_shell` | `true` | framework | Removes the app-side caption (S1). |
| `persist.waydroid.suspend` | `false` | Waydroid | The daemon owns the lifecycle; Android must not freeze itself. |
| `debug.sf.nobootanimation` | `1` | SurfaceFlinger | No boot animation. |

At boot the agent sets `waydroid.active_apps` to `crownos`, which makes the stock HWC mode selection pick multi-window.

## 12. `crownos-android-setup` command line

Runs as root. Every subcommand writes `setup.json` when it changes setup state.

| Command | Caller | Does |
|---|---|---|
| `crownos-android-setup run` | `crownos-android-setup.service` | The first-boot setup. It is idempotent and resumes after a reboot. |
| `crownos-android-setup prepare-container` | `ExecStartPre=` drop-in on `waydroid-container.service` | Pins binder nodes, binds vgem's render node for the hwcomposer's fences (`modules-load.d` loads vgem), and on the Venus tier adds the NVIDIA render node to the container. |
| `crownos-android-setup remove [--wipe-data]` | Settings, through `pkexec` (`org.crownos.android.setup`) | Removes the images and overlay, disables the units and deletes the marker. `--wipe-data` also deletes every user's Waydroid data. |
| `crownos-android-setup enable` | Settings, through `pkexec`, after a reset or to turn Android apps back on | Sets `enabled: true` in `android.ron`, creates the pending marker and starts `crownos-android-setup.service` without waiting for it. |
| `crownos-android-setup make-manifest --root <dir> --version <v> --pins <json>` | CI bundle build | Writes `manifest.json` for an unpacked bundle. |

## 6. `android.ron`

### System file

`/etc/crownos/android.ron` is read through `crownos_config::system::android::AndroidConfig` (pattern: `system/ai.rs`). The installer writes it, and the setup service updates `tier` and `bundle`. When the file is missing, Android support was not chosen.

```ron
(
    enabled: true,
    google_play: true,
    // Written by the setup service after probing. Absent until then.
    tier: Some(Native),        // Native | Venus | Software
    bundle: Some("2026.11.0"), // installed bundle version
)
```

### User section

The user section is `~/.config/crownos/android.ron`, declared with `section!` as `Android in "android", keys AndroidKey`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `idle_minutes` | `u32` | `5` | Stop Android this long after the last Android window closes. `0` means never. |
| `share_downloads` | `bool` | `true` | Bind `~/Downloads` into Android. |
| `share_pictures` | `bool` | `false` | Bind `~/Pictures` into Android. |

## 7. crownotify progress notifications

New method in `crownotify::proto` (appended, so existing selectors are unchanged):

```rust
/// Post or update a progress notification. `replaces_id` 0 posts a new one.
/// `percent: None` shows a spinner. The notification is resident and never
/// expires until replaced with `percent: Some(100)` and actions, or closed.
fn send_progress_notification(
    replaces_id: u32,
    app_name: String,
    icon: String,          // icon name or absolute path
    summary: String,
    body: String,
    percent: Option<u8>,
    actions: Vec<String>,  // id, label pairs, as in freedesktop Notify
) -> u32;
```

The D-Bus `Notify` path keeps reading the `value` hint as percent. Lane L7 also adds:
- **Resident notifications:** `resident` is honoured, so invoking an action does not close a resident notification.
- **Raising the app:** activating a notification with a `desktop-entry` raises that app's windows through crownui's app-icons API (`activate_app_icon`) before `ActionInvoked`. No `ActivationToken` signal is sent, because crownui 0.1.1 has no xdg-activation token API. The compositor treats crownotify's activation as raise-only (it never minimizes).

## 10. Naming and packaging

### Window identity
- **app_id:** every Android window's `app_id` is `waydroid.<package>`, which equals its desktop file id. See `android_protocol::app_id`.
- **Security context:** sandbox engine `id.waydro.waydroid`, app id `waydroid`, instance id = the session's UUID.

### Packages
- **`crownos-android`:**
  - Ships `/usr/bin/crown-androidd`, `/usr/bin/crown-android` and `/usr/lib/crownos-android/crownos-android-setup`.
  - Ships the bundle under `/usr/share/crownos-android/bundle/<version>/`, `channels.cfg` (unresolvable default channels, see the security note in the file) and the units below.
  - Depends on `waydroid lxc python-gbinder libgbinder libglibutil dnsmasq`.
- **`crownos-android-venus`:** the host render server and the Venus overlay files, installed only on the Venus tier.

### Units
| Unit | Kind | Purpose |
|---|---|---|
| `crownos-android-setup.service` | system, exec | first-boot setup, gated by `ConditionPathExists=/var/lib/crownos/android/pending` |
| `crown-androidd.socket`, `crown-androidd.service` | user | socket-activated daemon on `%t/crownos/crownos-android.sock` |
| `crown-androidd.path` | user | starts the daemon at login while the setup pending marker exists, so the loader shows with no client |
| `crownos-android-session.service` | user | runs `waydroid session start`, started and stopped by the daemon |
| `crownos-android-render.service` | user | Venus host render server, Venus tier only, started by the daemon before the session |

### Paths
| Path | Owner | Contents |
|---|---|---|
| `/var/lib/crownos/android/` | root | `pending`, `setup.json`, `staging/`, `images/<system>-<vendor>/`, `bundles/<version>/` |
| `~/.local/share/crownos/android/` | user | `XDG_DATA_HOME` of the Waydroid session, with `waydroid` symlinked to `~/.local/share/waydroid` |
| `$XDG_RUNTIME_DIR/crownos/android-wayland.sock` | user | security-context socket handed to Waydroid as `WAYLAND_DISPLAY` |
| `/dev/socket/crown_agent` | container | HWC ⇄ agent, mode 0660 root:system |
| `/data/crownos/agent.sock` | container | daemon ⇄ agent, mode 0600 owned by the host user's uid |

### Polkit actions
- `org.crownos.android.setup`: start or retry `crownos-android-setup.service`.
- `org.crownos.android.container`: start or stop `waydroid-container.service`.
- Both are granted without a password to active local sessions.
