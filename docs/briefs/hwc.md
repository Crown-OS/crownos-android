# Lane `hwc`: the CrownOS hwcomposer

## Repo and owned paths
**crownos-waydroid-hwc**, worktree `~/crownos-lanes/hwc/crownos-waydroid-hwc`. It is a fork of `waydroid/android_hardware_waydroid` (Android 13 branch `lineage-20`), and the branch starts from `crownos`. That commit already holds the out-of-tree NDK build in `ndk/` (read `ndk/README.md` and `docs/decisions/s2-hwc-build.md`). You own the whole repo.

## Goal
Every Android task becomes its own native `xdg_toplevel`, sized to the task, with zero-copy dmabuf layers and no stalls.

## Requirements

### 1. Task window mode
Rewrite the multi-window path (`modes/multi-window.cpp`, `wayland-hwc.cpp`, `hwcomposer.cpp`) into task windows. Keep the other modes for safe mode.
- **Display size:** the Android display has a fixed size from `persist.waydroid.width/height` (4096x4096, contract 11). An `xdg_toplevel.configure` NEVER resizes or hotplugs the Android display.
- **Grouping:** group layers by the `TID:<task>#<pkg>/<component>` layer name, as today.
- **Window per task:**
  - `app_id` `waydroid.<pkg>`;
  - title from the agent's `TaskInfo.label`, falling back to the package;
  - `set_min_size` from `TaskInfo` minimums, converted from pixels to logical;
  - server-side decorations requested through `zxdg_decoration_manager_v1`;
  - window geometry equals the task's slot size in logical units.
- **Layers:** each layer is a subsurface at `(displayFrame.left − slot.left, displayFrame.top − slot.top) / scale`. Clip to the slot with wp_viewporter.
- **Input:** surface-local coordinates map to Android display coordinates as `local × scale + slot origin`, for pointer, touch, tablet and relative pointer.
- **Configure flow:**
  1. On `configure(w, h)`, ack immediately and send agent-wire `SetSize(task, w×scale, h×scale, serial)`.
  2. Until `SlotApplied` with that serial arrives, keep showing the last buffers scaled to the new size through the viewport. No black frames, no size mismatch.
  3. On `SlotApplied`, use the new slot.
- **Close:** sends `Close(task)` to the agent. Keep a removeTask fallback through IWaydroidTask when the agent is not connected.
- **Focus:** keyboard focus sends `Focus(task)`.

### 2. Android shell protocol client
- **Binding:** bind `crownos_android_shell_manager_v1` when it is offered, and call `get_android_toplevel` before the first buffer commit. The XML is in `~/crownos-lanes/hwc/crownos-protocols/protocols/` (symlink to the `android/base` worktree); generate the C client code with wayland-scanner in the NDK build.
- **Per window:**
  - `set_task(task, pkg)`;
  - `set_resize_lock(fixed)` when `TaskInfo.resizable` is false, otherwise `none`;
  - `set_title_color(TaskInfo.color)`.
- **Back:** the `back` event sends `Back(task)` to the agent.

### 3. Non-task layers
- **IME and toasts:** the InputMethod layer and toast layers (layers without a TID that are not the cursor) become subsurfaces of the focused task window. The IME is anchored to the bottom and centred; toasts are bottom-centred above the IME.
- **Everything else:** other untagged layers are ignored, as today.
- **Separate windows:** never create a separate "InputMethod" toplevel.

### 4. Agent link (`/dev/socket/crown_agent`, agent-wire v1)
- **Codec:** write the agent-wire codec in C++17 as a self-contained header plus source, with no Android dependencies.
- **Host test:** build it for the host as well and test it against `contracts/agent-wire-vectors.txt` (encode every sample to the exact bytes, decode it back).
- **Connection:**
  - Send `Hello(wire 1, ro.crownos.bundle)` and check the agent's Hello.
  - On a mismatch, or when `persist.crownos.safe_mode` is `1`, fall back to stock multi-window behaviour.
  - Reconnect with backoff when the agent restarts.
  - Do all socket IO off the composition thread, with no blocking in `hwc_set`.
- **Activation:** `Activate(task, token)` calls `xdg_activation_v1.activate(token, surface)`. Bind xdg-activation.

### 5. Buffers and fences
- **dmabuf:** bind linux-dmabuf v4 and read default and per-surface feedback. Allocate and import only (format, modifier) pairs the compositor advertises. Fall back to wl_shm when an import is impossible. Never send an unadvertised import, because it kills the connection (`s4-venus.md` lists four fixes the forks needed).
- **Acquire fences:** no CPU `sync_wait` on the composition path. Use `wp_linux_drm_syncobj_manager_v1`: import each acquire sync_file into a timeline point on a syncobj created on the render node, and set acquire and release points per surface commit. Fall back to the current wait only when the global is missing.
- **Release and present fences:** no sw_sync dependency, because the ISO kernel has no `CONFIG_SW_SYNC`. Follow `s3-platform.md`: hand SurfaceFlinger vgem fences signalled when the compositor releases or presents, and use `-1` when vgem is absent.

### 6. Outputs and scale
- **Outputs:** track all `wl_output`s (v4). Android uses one density, from the highest preferred fractional scale across outputs. Windows on lower-scale outputs are downscaled by the compositor through the viewport.
- **Frame timing:** follow the presentation feedback of the output that shows the focused window.

### 7. Input quality
- **Scrolling:** high-resolution scroll (`axis_value120`) instead of quantized `REL_WHEEL` steps.
- **Pen:** the full pen pressure range, not `>>8`.
- **Pinch:** zwp_pointer_gestures pinch becomes a synthetic two-finger touch.
- **Keyboard:** map the xkb layout name to Android keyboard layouts with a real table, not the first word.

### 8. Window visibility
- **Suspended:** bind `xdg_wm_base` v6. On `suspended`, send `Visibility(task, false)` and stop committing to that window. When it clears, send true and attach the freshest buffer.

## Build and acceptance
- **Build:** `ndk/build.sh` builds `hwcomposer.waydroid.so` with `-Wall -Werror`. Run it through `~/crownos-lanes/bin/build-slot`.
- **Checks:** `check-symbols.sh`, `check-hidl-abi.sh` and `check-gralloc-handle.sh` stay green. Add new protocols to the build's scanner step.
- **Host tests:** the codec against the vectors, plus pure helper logic you can isolate (slot offset and input mapping, configure-serial tracking, modifier intersection) as host-built unit tests.
- **Artifact:** the built .so lands in `~/.cache/crownos-android/hwc/out/x86_64/vendor/lib64/hw/`. The setup lane picks it up from there.
- **Commits:** on `android/hwc`.
