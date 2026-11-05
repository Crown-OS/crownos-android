# Contracts

Every lane builds against these contracts and nothing else. A contract changes only through the lead, with a version bump and updated golden vectors.

| # | Contract | Status | Source of truth |
|---|---|---|---|
| 1 | `crownos-android-shell-v1` Wayland protocol | draft until S1/S2 | `crownos-protocols/protocols/crownos-android-shell-v1.xml` (Wave 1) |
| 2 | HWC ⇄ agent records | draft until S1/S2 | [agent-wire.md](agent-wire.md) |
| 3 | daemon ⇄ agent records | draft until S1 | [agent-wire.md](agent-wire.md) |
| 4 | `crownos-android` crownos-ipc service | frozen v1 | `crates/android-protocol` |
| 5 | `setup.json` | frozen v1 | `crates/setup-state` |
| 6 | `android.ron` (system and user) | frozen v1 | below; lane L8 adds it to crownos-config |
| 7 | crownotify progress notifications | frozen v1 | below; lane L7 adds it to `crownotify/src/proto.rs` |
| 8 | overlay bundle manifest | frozen v1 | `crates/bundle` (`Manifest`) |
| 9 | image pins and OTA channel | frozen v1 | `crates/bundle` (`ImagePin`, `OtaChannel`) |
| 10 | naming and packaging | frozen v1 | below |

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
- **Activation tokens:** an `ActivationToken` signal is emitted before `ActionInvoked`, with a token from crownui's xdg_activation.

## 10. Naming and packaging

### Window identity
- **app_id:** every Android window's `app_id` is `waydroid.<package>`, which equals its desktop file id. See `android_protocol::app_id`.
- **Security context:** sandbox engine `id.waydro.waydroid`, app id `waydroid`, instance id = the session's UUID.

### Packages
- **`crownos-android`:**
  - Ships `/usr/bin/crown-androidd`, `/usr/bin/crown-android` and `/usr/lib/crownos-android/crownos-android-setup`.
  - Ships the bundle under `/usr/share/crownos-android/bundle/<version>/`, `channels.cfg` and the units below.
  - Depends on `waydroid lxc python-gbinder libgbinder libglibutil dnsmasq`.
- **`crownos-android-venus`:** the host render server and the Venus overlay files, installed only on the Venus tier.

### Units
| Unit | Kind | Purpose |
|---|---|---|
| `crownos-android-setup.service` | system, oneshot | first-boot setup, gated by `ConditionPathExists=/var/lib/crownos/android/pending` |
| `crown-androidd.socket`, `crown-androidd.service` | user | socket-activated daemon on `%t/crownos/crownos-android.sock` |
| `crownos-android-session.service` | user | runs `waydroid session start`, started and stopped by the daemon |
| `crownos-android-render.service` | system | Venus host render server, Venus tier only |

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
