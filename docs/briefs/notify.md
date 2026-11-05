# Lane `notify`: crownotify progress loader and app raising

## Repo and owned paths
**crownotify**, worktree `~/crownos-lanes/notify/crownotify`, branch `android/notify`. You own the whole repo. crownotify takes crownui 0.1.1 from crates.io; do NOT switch it to a path dependency.

## Goal
1. The setup loader notification: a spinner or a percentage, resident, updated in place.
2. Clicking any notification raises its app's windows, so a click on an Android notification brings the Android window forward.

## Requirements
1. **Contract 7.** Append to `src/proto.rs` (selectors hash the signature, so existing clients keep working):
   ```rust
   fn send_progress_notification(replaces_id: u32, app_name: String, icon: String, summary: String, body: String, percent: Option<u8>, actions: Vec<String>) -> u32;
   ```
   - `replaces_id` 0 posts a new notification; otherwise the existing one is replaced in place, keeping its position (`Store::post`).
   - `actions` are freedesktop-style id/label pairs.
   - The notification is resident and never expires, and it is excluded from Do Not Disturb suppression for its own updates.
   - Add `close_notification` handling for it if it is missing.
2. **Indeterminate progress.** Change the model so `Content::Progress` carries `percent: Option<u8>`, where `None` means indeterminate. `ui/card/general.rs` shows `crownui::kit::spinner()` for None and the existing `ui/widget/progress.rs` bar for Some. Use `keyed` so switching between them doesn't rebuild the card.
3. **Resident.** `Notification.resident` is honoured: invoking an action on a resident notification does not close it (`daemon/state.rs::invoke_action`). This also applies to D-Bus Notify with the `resident` hint.
4. **Raising the app.** When the user activates a notification (the card body or any action) that has a `desktop-entry` (`AppIdentity.desktop_entry`), ask the compositor to raise that app with `cx.activate_app_icon(<desktop entry id>)`. That is crownui's app-icons API, which crowndock uses; check that it exists in crownui 0.1.1 (`~/.cargo/registry/src/*/crownui-0.1.1/view/src/app_icons.rs`). Then emit `ActionInvoked` as today.
5. **No fake tokens.** Do not emit `ActivationToken` without a real token.

## Acceptance
- **IPC tests:** `tests/ipc_service.rs` covers the new method (post, replace in place, resident survives an action, close).
- **D-Bus tests:** `tests/freedesktop.rs` covers a resident notification surviving an action.
- **Unit tests:** the Progress model mapping (`value` hint → Some, the new method's None → spinner).
- **Render:** `examples/render.rs` still builds, plus a spinner variant if the example harness supports it.
- **Gates:** green with the features crownotify's CI uses (check its README and CI workflow). **Commits:** on `android/notify`.
