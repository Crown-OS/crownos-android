# Lane `compositor`: crownpositor support for Android windows

## Repos and owned paths
- **crownpositor**, worktree `~/crownos-lanes/compositor/crownpositor`. You may edit `protocols/src/`, `protocols/tests/`, `compositor/src/` and the Cargo manifests.
- **crownos-protocols**, worktree `~/crownos-lanes/compositor/crownos-protocols`. It starts from `android/base`, which already has `crownos-android-shell-v1.xml` and the `crownos_protocol!(path, xdg_shell)` macro form. Do not change the XML. You may add tests.

## Goal
The `xdg_toplevel`s of the Android runtime must behave exactly like native windows: tile, float, snap, move, resize, maximize, fullscreen, minimize and close. They also need the Android extras.

## Requirements

### 1. Protocol server
Add `crownos-android-shell-v1` in `protocols/src/crownos_android_shell/`.
- **Templates:** follow `crownos_app_icons/` for the global with a client filter and the dispatch bundle trait, and `xdg_toplevel_drag.rs` for per-xdg_toplevel extension state.
- **State:**
  - Keep per-object state in the protocol State, not in user data.
  - `set_task`, `set_resize_lock`, `set_aspect` and `set_title_color` are double-buffered and apply on the toplevel's next commit. Hook commit the way `xdg_toplevel_drag` or the compositor's commit handler does.
- **Errors:**
  - A second `get_android_toplevel` for the same toplevel posts `already_extended`.
  - `mark_placeholder` from anyone but the daemon posts `not_allowed`.
- **Wiring:** in `compositor/src/handlers/` and `state/wayland.rs`.

### 2. Trust
Today `utils/privilege.rs` treats every security-context client as unprivileged, and `SecurityContext` fields are never read (`handlers/security_context.rs`).
- **When the manager is visible:**
  - **(a) The hwcomposer client:** the client came through a security context with `sandbox_engine == "id.waydro.waydroid"`, and that context's `creator_client_id` is a live, unsandboxed client whose executable (`/proc/<pid>/exe` from the client's credentials) is `crown-androidd`. Match on file name `crown-androidd`, so dev builds also qualify.
  - **(b) The daemon itself:** the unsandboxed client whose executable is `crown-androidd`. Only (b) may call `mark_placeholder`.
- **Implementation:** put the identity check in a small, unit-tested module (for example `utils/android_identity.rs`). A sandboxed Waydroid client keeps every other privilege restriction it has today.

### 3. Window behaviour for toplevels with an android extension
- **Server-side decorations:** the HWC requests SSD through xdg-decoration. Make sure the titlebar shows for Android windows in every state where native SSD windows show one.
- **Titlebar Back button:** add a Back button to the titlebar of Android windows only (`compositor/src/shell/decoration/`, `rendering/decoration/`, `input/decoration.rs`). It sends the `back` event. Also add a keybind action `AndroidBack` (Alt+Left by default) that sends `back` to the focused Android window. The mouse side button (BTN_SIDE) over an Android window sends `back` too, instead of forwarding the button.
- **Resize lock:**
  - `fixed`: the window floats at its committed size and cannot be resized. `auto_float` already floats when min == max; extend it to cover the lock.
  - `aspect`: resizes keep the ratio. Constrain in `resize_floating` and in the tiling sizes, the same way min/max size are honoured.
- **Title color:** a non-zero alpha tints the titlebar background, and the text stays readable by picking light or dark text from luminance.
- **Window rules:** `waydroid.*` app_ids must not get the special treatment stock Waydroid windows needed. Make sure nothing in `auto_float` or `is_splash` misclassifies them.

### 4. Suspended state, for every toplevel
- **What to send:** when `Shell::surface_visibility` says a toplevel is hidden (minimized, on a hidden workspace, locked screen) or fully occluded, send `xdg_toplevel` state `Suspended` to clients that bound xdg_wm_base v6 or later. Clear it when the toplevel becomes visible.
- **How:** reuse the visibility refresh in `lib.rs`/`state/visibility.rs` and the existing configure path. Do not send a configure when nothing changed.

### 5. Placeholder handoff
- **Marking:** `mark_placeholder(toplevel, app_id)` marks the daemon's splash toplevel.
- **Handoff:** when a toplevel from an android-shell client maps with the same app_id within 120 s:
  - it takes the placeholder's place: workspace, tile slot or floating rect, and its Maximized or Fullscreen state;
  - it skips its own launch animation (the splash already played it);
  - the compositor sends `xdg_toplevel.close` to the placeholder.
- **Expiry:** the mark expires after 120 s.
- **Code:** pure logic in its own module with unit tests. Reuse the `Launches` pattern in `crownos_app_icons/launches.rs` for the matching window.

## Acceptance
- **Protocol integration tests** (pattern: `protocols/tests/xdg_toplevel_drag.rs`, with fake ClientData carrying a fake SecurityContext):
  - the global is visible to the allowed client and invisible to an ordinary sandboxed client;
  - `already_extended` and `not_allowed` are posted;
  - double-buffered state applies on commit.
- **Unit tests:** identity check, aspect constraint, placeholder matching and expiry, suspended-state diffing, title text contrast.
- **Gates:** green for the crownpositor workspace and for crownos-protocols with `--features server,client`.
- **Commits:** on `android/compositor` in both worktrees.
