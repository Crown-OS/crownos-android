# S1: Android windowing control on the stock image

Status: static analysis done, runtime half pending approval. Date: 2026-11-05.

## Question

Can an outside process put every Android task of one Waydroid session into a non-overlapping atlas slot, size it, launch into it, focus it and remove it, with no caption bar, no shadows and no framework rebuild, using only Waydroid overlay files, props and scripts on the pinned image?

Pinned image: LineageOS 20 GAPPS, `TQ3A.230901.001`, `userdebug`, `test-keys`, built 2026-04-03 (`/system/build.prop`). The WM Shell in SystemUI is the Android 13 QPR3 version: it has `DesktopModeWindowDecoration`, and the server's `Task.updateShadowsRadius` no longer exists.

## Criteria and verdicts

| Id | Criterion | Verdict |
|----|-----------|---------|
| G1 | An outside process can set freeform task bounds with no framework changes | **GO** (static). The runtime test confirms it. |
| G2 | At least one caption-removal option that does not patch framework.jar | **GO** (static). Four options exist. Which one ships depends on the runtime test. |
| G3 | Resize applies within 250 ms | **NEEDS-RUNTIME** |
| G4 | Tasks in non-overlapping slots all stay visible and resumed | **NEEDS-RUNTIME** (static evidence is favourable) |
| G5 | The agent can start at boot from an overlay init.rc as root | **GO** (static). The runtime test confirms it. |

## Method

- I extracted the images with `debugfs` into `~/.cache/crownos-android/s1/extract/`: `framework.jar`, `services.jar`, `framework-res.apk`, `SystemUI.apk` (with its `oat/`), `TrebuchetQuickStep.apk`, `Shell.apk`, `init`, the rc files and the test app APKs.
- I disassembled the jars with baksmali 2.5.2 into `~/.cache/crownos-android/s1/smali/{framework,services,systemui}`.
  - baksmali 2.5.2 crashes on Android 13 hidden-API flags and skipped 693 framework classes, including `ViewRootImpl`.
  - The fix was to re-disassemble copies of the framework dex files with their `HIDDENAPI_CLASS_DATA` map entry removed (`~/.cache/crownos-android/s1/dex/framework/*.stripped`). No classes are missing now.
- Resources and manifests come from `aapt2 dump`. Signing certificates come from `apksigner verify --print-certs`.
- I read the Waydroid patches in `android_vendor_waydroid/waydroid-patches/base-patches-33`, the lineage-20 HWC, and the host tools in `/usr/lib/waydroid/tools`.

## Findings

### a. Where the freeform caption is drawn, and how to remove it

**The default on this build is the legacy app-side caption.** `DecorView.createDecorCaptionView` in framework.jar builds a `DecorCaptionView` inside every app process when all of these hold:

```
!mWindow.isFloating() && isApplication(type 1|2|4)
  && windowConfiguration.hasWindowDecorCaption()
  && !ViewRootImpl.CAPTION_ON_SHELL          ; sget-boolean ViewRootImpl->CAPTION_ON_SHELL, if-nez -> no caption
```

- `ViewRootImpl.<clinit>` sets `CAPTION_ON_SHELL = SystemProperties.getBoolean("persist.wm.debug.caption_on_shell", false)`.
  - Zygote reads it once, when it preloads the class. A change needs an Android restart.
  - The same flag gates `DecorView.notifyCaptionHeightChanged` and `onWindowDragResizeStart`, plus `InsetsController.{setCaptionInsetsHeight,updateState,onStateChanged,captionInsetsUnchanged}` and `InsetsSource.calculateInsets`.
- `WindowConfiguration.hasWindowDecorCaption()` returns `activityType == STANDARD && (windowingMode == FREEFORM(5) || displayWindowingMode == FREEFORM(5))`.
  - **So on a freeform display, MULTI_WINDOW tasks get the legacy caption too.**
- Waydroid's caption buttons all live in this legacy view:
  - 0018: minimize and PiP.
  - 0019: PiP only when the activity supports it.
  - 0020: back.
  - 0021: graphics.
  - 0022: minimize calls the `vendor.waydroid.window@1.0::IWaydroidWindow.minimize(pkg)` HIDL in the HWC.

**WM Shell caption in SystemUI.** `CaptionWindowDecorViewModel` is chosen because `DesktopModeStatus.isAnyEnabled()` is false (`persist.wm.debug.desktop_mode` and `desktop_mode_2` both default to false).

- `FreeformTaskListener` (shell listener type -5, for FREEFORM) calls `onTaskOpening`, which creates a `CaptionWindowDecoration` when `mode == FREEFORM || (STANDARD && displayMode == FREEFORM)`.
  - This happens because `Transitions.ENABLE_SHELL_TRANSITIONS = persist.wm.debug.shell_transit`, which defaults to false.
- `WindowDecoration.relayout` always creates these layers:
  - `TID:<id>#<pkg>/DecorContainer`
  - `.../Background`: an effect layer with `setShadowRadius(freeform_decor_shadow_{focused,unfocused}_thickness)`, which is 20 dp or 5 dp.
  - `.../CaptionContainer`
  - The `TID:` names come from patch 0007.
- **The caption is shown only `if (ViewRootImpl.CAPTION_ON_SHELL)`**, and then it adds `ITYPE_CAPTION_BAR` insets through `wct.addRectInsetsProvider`. Otherwise it runs `startT.hide(mCaptionContainerSurface)`.
- `CaptionWindowDecoration.relayout` also outsets the decor by `freeform_resize_handle`. It installs a `DragResizeInputListener` (Android-side resize handles: 30 dp ring and 44 dp corners) for every visible freeform task, even when the caption is hidden.
- The SystemUI dimens are:

| Resource id | Name | Value |
|-------------|------|-------|
| `0x7f07023f` | `freeform_decor_caption_height` | 42 dp |
| `0x7f070242` | `..._shadow_focused_thickness` | 20 dp |
| `0x7f070243` | `..._shadow_unfocused_thickness` | 5 dp |
| `0x7f070244` | `freeform_resize_corner` | 44 dp |
| `0x7f070245` | `freeform_resize_handle` | 30 dp |

**Caption-removal options that do not patch framework.jar:**

| # | Option | What it needs | Notes |
|---|--------|---------------|-------|
| 1 | `persist.wm.debug.caption_on_shell=true` plus **the agent registered as the TaskOrganizer** | A prop, plus agent code (`organize` in the spike) | No APK changes. See c. for why this works. The agent must position, crop and show task leashes itself. |
| 2 | `caption_on_shell=true` plus a **SystemUI RRO** that sets caption height, both shadow thicknesses, resize handle and resize corner to 0 dp | A prop and a preinstalled overlay APK (`overlays/systemui-nodecor`) | SystemUI's `resources.arsc` has no `RES_TABLE_OVERLAYABLE` chunk, so a `/product/overlay` RRO may overlay any resource (idmap2 `CheckOverlayable`, default partition policy). No change to SystemUI.apk and no oat problem. Needs a runtime check that a 0-height caption view host behaves. |
| 3 | `caption_on_shell=true` plus a **SystemUI.apk smali patch** | Patch `WindowDecoration.relayout` (replace `sget-boolean v4, ViewRootImpl->CAPTION_ON_SHELL` with `const/4 v4, 0x0`) and drop the `DragResizeInputListener` creation in `CaptionWindowDecoration.relayout`. Re-sign. Whiteout over `system_ext/priv-app/SystemUI/oat/x86_64/SystemUI.{odex,vdex}`. | SystemUI is signed with the **public AOSP test platform key** (SHA-256 `c8a2e9bc…2ab8`), so re-signing is possible. This is the plan's original option 1, now a fallback. |
| 4 | **WINDOWING_MODE_MULTI_WINDOW tasks** | Either `caption_on_shell=true`, or display windowing mode FULLSCREEN (`IWindowManager.setWindowingMode(0, 1)`, which needs `INTERNAL_SYSTEM_WINDOW`). Also agent leash handling. | The shell has no listener for type -3, so there is no shell decor, shadow or resize handle. `DecorView.updateElevation` sets elevation only in FREEFORM, so there is no client shadow. **But nobody repositions the leash** (see c.). |
| 5 | A framework-res RRO that makes the `decor_caption` layout 0 high (untested idea) | A preinstalled RRO targeting `android` (no overlayable, so allowed) | Keeps `DecorCaptionView` and its drag-on-touch code. Weakest option. |

A prop alone is not enough. With `caption_on_shell=true` and nothing else, the WM Shell caption appears in place of the legacy one.

### b. The Waydroid Android 13 patches

**frameworks/base**

- **0007** names surfaces after their task, so the HWC can group layers per task:
  - Every window surface becomes `TID:<taskId>#<title>` (`WindowStateAnimator`).
  - The snapshot animation leash becomes `TID:<id>#<pkg>/SnapshotAnim` (`SurfaceFreezer`).
  - WM Shell decor surfaces become `TID:<id>#<pkg>/{DecorContainer,Background,CaptionContainer}`.
  - The splash screen becomes `<pkg>/SplashScreen` and the snapshot window `<pkg>/SnapshotStartingWindow`.
- **0011** names SurfaceViews `TID:<taskId>#<title>` (and `...$Background`), and makes `SurfaceControl.toString()` return the bare name.
  - The HWC splits names as `TID:<tid>#<aid>/<component>#...` (`waydroid_mode.cpp split_layer_name`).
- **0008** makes `AppTransition.loadAnimation` return null: all app transition animations are off.
- **0012** sets `minLeft = 0` in `TaskPositioner.updateWindowDragBounds` (confirmed in smali: `Math.max(left+dx, 0)`). It only affects server-side caption drags. The atlas never uses them.
- **0013** lowers `DECOR_SHADOW_FOCUSED_HEIGHT_IN_DIP` from 20 to 5. Smali: `updateElevation` uses 5.0f for both focused and unfocused.
- **0015** makes F11 in `DecorView.dispatchKeyEvent` (keycode `0x8d`) call `WindowControllerCallback.toggleFreeformWindowingMode()`, which switches the task between freeform and fullscreen.
- **0018 to 0022** are the legacy caption buttons described in a. 0022 adds `vendor.waydroid.window-V1.0-java` to framework.jar.
- **0028** changes the `PhoneWindow` default of `Settings.Global.render_shadows_in_compositor` from 1 to 0 (smali: `getInt(..., "render_shadows_in_compositor", 0)`). With 0, `DecorView.updateElevation` gives freeform windows a 5 dp elevation. That draws a client-side shadow and adds `surfaceInsets`, so the buffer is larger than the task bounds.
  - **To turn the shadow off:** `settings put global render_shadows_in_compositor 1`, then restart the app (the value is read in the `PhoneWindow` constructor).
  - On this build the server draws no task shadows (`Task.resetSurfaceControlTransforms` only ever sets 0). The only other shadow is the WM Shell `Background` layer, which options 1 to 3 remove.
- **0029** re-implements PolicyControl with the `Settings.Global` key **`policy_control`** (`Settings.Global.POLICY_CONTROL`).
  - `immersive.full=*`: the status bar and navigation bar get height 0 (`NavigationBar`, `StatusBarWindowController`), and their insets are forced hidden (`InsetsState.shouldForceHide`, `SystemBarUtils.isImmersive`).
  - `immersive.status=*`: only the status bar is hidden. 0031 then strips HOME, RECENT and the home handle from the nav bar.
  - Any other value: nothing changes.
- 0032 is related: Back on a task root does not finish the task unless `waydroid.active_apps == "Waydroid"` (full UI).

**frameworks/native**

- **0006**: `HidlComposer` talks to `vendor.waydroid.display@1.0-1.2`. Each layer's name, buffer format, stride and size are sent to the HWC (`setLayerName`, `setLayerHandleInfo`, `setLayerSize`), keyed by Z order. It also sends the client target's info, and passes cursor metadata (`org.chromium.arc.V1_0.CursorInfo`) through `setMouseMetadata`.
- **0012**: when `persist.waydroid.multi_windows=true`, `Output::ensureOutputLayerIfVisible` no longer drops layers whose visible region or draw region is empty. Occluded windows still reach the HWC.
- 0013 (also relevant): layer metadata is copied to the parenting layer.

### c. Hidden API signatures on this build, and how WM Shell listeners behave

These come from the smali `.method` lines of the `$Stub` interfaces and classes.

**`IActivityTaskManager`** (transaction codes from `$Stub`)

| Method | Signature | Code |
|--------|-----------|------|
| `resizeTask` | `(int taskId, Rect bounds, int resizeMode) -> boolean` | 0x31 |
| `setFocusedTask` | `(int taskId)` | 0x15 |
| `moveTaskToFront` | `(IApplicationThread, String callingPackage, int taskId, int flags, Bundle options)` | 0x19 |
| `removeTask` | `(int taskId) -> boolean` | 0x16 |
| `registerTaskStackListener` / `unregisterTaskStackListener` | `(ITaskStackListener)` | 0x2e / 0x2f |
| `getTasks` | `(int maxNum, boolean filterOnlyVisibleRecents, boolean keepIntentExtra, int displayId) -> List<RunningTaskInfo>` | 0x18 |
| `getWindowOrganizerController` | `() -> IWindowOrganizerController` | 0x42 |
| `startActivityAsUser` | `(IApplicationThread, String, String, Intent, String, IBinder, String, int, int, ProfilerInfo, Bundle, int)` | |
| `setTaskResizeable` | `(int, int)` | |

**`IWindowOrganizerController`**

- `applyTransaction(WindowContainerTransaction)`
- `applySyncTransaction(WindowContainerTransaction, IWindowContainerTransactionCallback) -> int`
- `getTaskOrganizerController()`
- The public wrapper is `WindowOrganizer.applySyncTransaction(WCT, WindowContainerTransactionCallback)`.

**`WindowContainerTransaction`** (public no-argument constructor)

- `setBounds(WindowContainerToken, Rect)`
- `setWindowingMode(WindowContainerToken, int)`
- `reorder(WindowContainerToken, boolean)`
- `setDensityDpi(WindowContainerToken, int)` (present)
- Also present: `setAppBounds`, `setScreenSizeDp`, `setSmallestScreenWidthDp`, `setFocusable`, `setHidden`, `setBoundsChangeTransaction`, `startTask`, `removeTask`.

**`ActivityOptions`**

- `setLaunchBounds(Rect) -> ActivityOptions`
- `setLaunchWindowingMode(int)`
- `setLaunchCookie(IBinder)`
- `setLaunchDisplayId(int)`, `setLaunchRootTask(WindowContainerToken)`

**`TaskInfo` fields:** `token`, `configuration` (`windowConfiguration.getBounds/getWindowingMode`), `positionInParent`, `isVisible`, `isFocused`, `isResizeable`, `resizeMode`, `supportsMultiWindow`, `minWidth`, `minHeight`, `defaultMinSize`, `taskDescription` (label, primary, background and status-bar colors, min width and height), `topActivityInfo.windowLayout`, `launchCookies`.

**Callback classes the agent subclasses:**

- `TaskStackListener`: public constructor. `onTaskCreated(int, ComponentName)`, `onTaskMovedToFront(RunningTaskInfo)` and the rest.
- `TaskOrganizer`: `registerOrganizer() -> List<TaskAppearedInfo>` and `onTaskAppeared/InfoChanged/Vanished`.
- `WindowContainerTransactionCallback.onTransactionReady(int, SurfaceControl.Transaction)`.

**How ShellTaskOrganizer handles root tasks here.**

- `taskInfoToTaskListenerType` maps FULLSCREEN to -2, PINNED to -4, FREEFORM to -5 and MULTI_WINDOW to -3.
- Only -2 (`FullscreenTaskListener`), -4 (`PipTaskOrganizer`) and -5 (`FreeformTaskListener`) are registered. These were found in R8-merged lambdas that call `addListenerForType`.
- **A MULTI_WINDOW root task that WM creates from `ActivityOptions` has no listener.** `ShellTaskOrganizer.onTaskAppeared` stores it and nobody touches its leash.

**Server side (services.jar).**

- `Task.canBeOrganized()` is `isRootTask() || mCreatedByOrganizer || parent created by organizer`, with no windowing-mode filter. **Every root task is organized** by the last registered organizer (`TaskOrganizerController.getTaskOrganizer()` is `mTaskOrganizers.peekLast()`).
- For organized tasks, `updateSurfacePositionNonOrganized()` and `updateSurfaceSize()` return early.
- `Task.showSurfaceOnCreation()` returns false. `Task.prepareSurfaces` still calls `setVisibility(leash, isVisible)`.
- **So after a bounds change, only the organizer moves and crops a task leash.**
  - In stock FREEFORM, that is `WindowDecoration.relayout` (`finishT.setPosition(mTaskSurface, positionInParent)` and `setCrop`).
  - For MULTI_WINDOW, nobody does it: the slot origin must never change, or the agent must organize.
- `registerTaskOrganizer` appends to a `LinkedList` and moves every non-organizer-created root task to the newest organizer. The previous organizer gets `onTaskVanished`. When the newest organizer dies or unregisters, the tasks go back.
  - **This is how option 1 works:** an agent that registers after SystemUI owns all app and home tasks. No WM Shell decoration, shadow layer or drag-resize handle is ever created.
  - The agent must then position, crop and show leashes. It gets no splash windows (the `addStartingWindow` default is empty) and no PiP or split handling.
  - After a SystemUI restart, SystemUI re-registers and takes the tasks back. The agent sees `onTaskVanished` for tasks that still exist and re-registers (`LeashOrganizer.reclaim`).
- `resizeTask` additionally requires `WindowConfiguration.canResizeTask()`, which is true for FREEFORM or MULTI_WINDOW. Mode bit 0 is preserve-window.
- `TaskDisplayArea.isValidWindowingMode` accepts MULTI_WINDOW and FREEFORM only for activities that support multi-window. A non-resizable app needs `force_resizable_activities` or `enable_non_resizable_multi_window` (both are observed by `WindowManagerService$SettingsObserver`).
- The display windowing mode (`DisplayWindowSettings.getWindowingModeLocked`) is FREEFORM when `mSupportsFreeformWindowManagement && (mIsPc || forceDesktopMode)`.
  - `mIsPc` comes from `android.hardware.type.pc`. `init.waydroid.rc` bind-mounts `/system/etc/hidden_xml/pc.xml` over the empty `/system/etc/permissions/pc.xml` when `persist.waydroid.multi_windows=true`.
  - `android.software.freeform_window_management.xml` is always present.

### d. Permission checks

| Call | Check in services.jar | uid 0 (root) | uid 2000 (shell) |
|------|----------------------|--------------|------------------|
| `resizeTask`, `setFocusedTask`, `registerTaskStackListener`, `IWindowOrganizerController.applyTransaction/applySyncTransaction`, `ITaskOrganizerController.registerTaskOrganizer` | `ActivityTaskManagerService.enforceTaskPermission` → `MANAGE_ACTIVITY_TASKS` (deprecated `MANAGE_ACTIVITY_STACKS` also accepted) | pass | pass |
| `moveTaskToFront` | `enforceCallingPermission(REORDER_TASKS)` | pass | pass |
| `removeTask` | `enforceCallingPermission(REMOVE_TASKS)` | pass | pass |
| `getTasks` | `isGetTasksAllowed` (`REAL_GET_TASKS` for other apps' tasks) | pass | pass |
| `startActivityAsUser` | `assertPackageMatchesCallingUid(callingPackage)` | pass, any package | pass only with `com.android.shell` |
| ActivityOptions | `SafeActivityOptions.checkPermissions` restricts only PINNED launch mode, `launchTaskId` (`START_TASKS_FROM_RECENTS`), display id, TDA, lock-task, remote animation and similar. Not launch bounds, FREEFORM or MULTI_WINDOW. | pass | pass |
| `IWindowManager.setWindowingMode(display, mode)` | `checkCallingPermission(INTERNAL_SYSTEM_WINDOW)` | pass | pass |
| `INotificationManager.enqueueTextToast` | `checkCallerIsSameApp(pkg)` | pass only with package `root` | pass with `com.android.shell` |
| `setTaskResizeable` | no permission check at all | pass | pass |

Why root and shell pass:

- **Root:** `ActivityManager.checkComponentPermission` grants everything to appId 0 and 1000. `ActivityTaskManagerService.isSameApp` returns true for uid 0 and 1000 before it looks at the package.
- **Shell:** `Shell.apk` (`com.android.shell`, platform-signed) requests `MANAGE_ACTIVITY_TASKS`, `REMOVE_TASKS`, `REORDER_TASKS`, `REAL_GET_TASKS`, `INTERNAL_SYSTEM_WINDOW`, `START_TASKS_FROM_RECENTS`, `START_ACTIVITIES_FROM_BACKGROUND` and `WRITE_SECURE_SETTINGS`.
- **Precedent:** the stock `vendor.waydroid.task@1.0-service` (`user system`) already calls `IActivityTaskManager.setFocusedTask` and `removeTask` from outside system_server.

The scrcpy FakeContext pattern applies only to the package name: pass `com.android.shell` as `callingPackage`, or `root` for toasts under uid 0. The agent needs no `Context` for task control. It reads labels through an `AssetManager` on `ApplicationInfo.sourceDir`.

### e. Disabling the wallpaper and using a blank HOME

- **Launcher:** `com.android.launcher3/com.android.launcher3.uioverrides.QuickstepLauncher` (TrebuchetQuickStep, `/system/system_ext/priv-app`). It also provides Recents: `config_recentsComponentName = com.android.launcher3/com.android.quickstep.RecentsActivity`.
- **Wallpaper:** `SystemServer` starts `WallpaperManagerService` only if `config_enableWallpaperService` (`0x01110148`) is true. `SystemServiceRegistry$58` hands out `DisabledWallpaperManager` when it is false.
  - framework-res has no overlayable chunk, so a preinstalled static RRO can set it to false (`overlays/framework-nowallpaper`, `isStatic=true`, so zygote applies it).
  - I found no prop for this.
- **Blank HOME:** nothing on the image is suitable. Settings' `FallbackHome` polls every 500 ms and shows the wallpaper.
  - `overlays/blank-home` is a code-free APK. Its HOME activity is `android.app.Activity` with a floating, translucent, empty theme that does not show the wallpaper, so it holds no display-sized buffer.
  - Select it with `cmd package set-home-activity org.crownos.spike.home/android.app.Activity` (present in `PackageManagerShellCommand`).
  - Then run `pm disable-user --user 0 com.android.launcher3` so neither HOME nor Recents runs the launcher.

### f. Overlay mount timing and the init.rc syntax

`container_manager.start()` runs `helpers.images.mount_rootfs()` (`images.py:162-176`):

1. Mount `system.img` on `/var/lib/waydroid/rootfs`.
2. Overlay-mount `lowerdir=/var/lib/waydroid/overlay:rootfs` on `rootfs` (upper `overlay_rw/system`, read-only).
3. Mount `vendor.img`, overlaid with `overlay/vendor`.
4. Bind `waydroid.prop`.
5. Only after that, `helpers.lxc.start()` runs `lxc-start ... -- /init` (`container_manager.py:214-218`; `lxc.rootfs.path = /var/lib/waydroid/rootfs`, `lxc.init.cmd = /init`).

So **init parses `/system/etc/init/*.rc` from the merged overlay**. The image itself ships `init.waydroid.rc` and `vendor.waydroid.task@1.0-service.rc` there.

- `overlay/system/etc/init/x.rc` appears as `/system/etc/init/x.rc`, and `overlay/system/product/...` as `/product/...`.
- If an overlay mount fails, `mount_overlays` is silently set to False (`images.py:173-176`). `root-steps.sh` re-enables it.
- SELinux is disabled in Waydroid, so no `seclabel` is needed.
- This init has no "No user specified" error, but the spike sets `user root` explicitly. `setenv` is supported. `/system/bin/app_process64` exists.

The Android 13 syntax used in `init/crown-agent-spike.rc`:

```
service crown_agent_spike /system/bin/app_process64 / org.crownos.agent.Spike serve /data/local/tmp/crown-agent-spike
    class late_start
    user root
    group root system shell
    setenv CLASSPATH /system/framework/crown-agent-spike.jar
    disabled
    oneshot
on property:sys.boot_completed=1
    start crown_agent_spike
```

For production, drop `oneshot` so init restarts the agent, and consider starting it on `init.svc.zygote=running`. The agent already waits for `activity_task` (`Services.awaitBinder`).

**Shell access.** The base props set `ro.adb.secure=1` and `ro.debuggable=0`, and adbd always drops to uid 2000 (adb patch 0002: `adb root` answers "not available in Waydroid"). There are three ways in:

1. Accept the authorization dialog in Android.
2. Pre-authorize the host key: `root-steps.sh` step 10 appends `~/.android/adbkey.pub` to `<waydroid data>/misc/adb/adb_keys`, owned 1000:2000 with mode 0640. Waydroid does not remap uids.
3. `sudo waydroid shell` (lxc-attach, root).

The runtime test avoids host sudo: root commands go through the init-started agent's FIFO (`serve` mode).

### HWC facts that matter to the plan

- The lineage-20 multi-window mode (`modes/multi-window.cpp`) creates one window per `TID:` key, plus one for the raw name `InputMethod`. **Every other untagged layer is dropped:** toasts, status and navigation bars, wallpaper.
- `prepare()` turns `HWC_FRAMEBUFFER` into `HWC_OVERLAY`. Layers that SurfaceFlinger forces to client composition (shadows, blur, color transforms) arrive with `HWC_SKIP_LAYER` and are not presented. **They also make SurfaceFlinger render a display-sized client target.** Keep shadowRadius at 0 everywhere.

## Recommended path

1. **Props and settings**
   - Set `persist.waydroid.multi_windows=true`. It drives SF patch 0012, the HWC mode and FEATURE_PC, which makes the display freeform. It can go in waydroid.cfg `[properties]` plus `waydroid upgrade --offline`, or be set with `waydroid prop set`.
   - Set `persist.wm.debug.caption_on_shell=true` to remove the legacy caption and its Waydroid buttons.
   - Run `settings put global render_shadows_in_compositor 1` to remove client shadows and surfaceInsets.
   - Run `settings put global policy_control immersive.full=*` so there are no bar insets.
   - Consider `settings put global enable_non_resizable_multi_window 1` for non-resizable apps.
2. **Caption path: option 1.** The agent registers as the TaskOrganizer.
   - It keeps tasks in FREEFORM and sizes them with `WindowContainerTransaction.setBounds`. It uses `applySyncTransaction` for atomic resize and applies the returned transaction.
   - It positions, crops and shows each leash from `TaskInfo.positionInParent` and the bounds. Cropping to the task bounds also clips any leftover client shadow out of neighbouring slots.
   - It reclaims tasks after SystemUI restarts.
   - No SystemUI or framework change ships.
3. If option 1 has side effects that cannot be accepted (IME, back, PiP, compat UI, SystemUI crash flicker), fall back to **option 2**: the static SystemUI RRO with caption height, both shadow thicknesses, resize handle and resize corner at 0, and the shell kept as organizer.
   - Next fallback is **option 3** (smali patch re-signed with the AOSP test platform key, plus an oat whiteout).
   - The last resort is framework.jar (plan option 3).
4. **MULTI_WINDOW (option 4)** is not needed. Keep it as a fallback only together with option 1, because without an organizer nobody moves its leash.
5. Disable the wallpaper with the static framework RRO. Use the code-free blank HOME with `set-home-activity`, and run `pm disable-user com.android.launcher3`.
6. The agent starts from the overlay init.rc as root. The HWC and the agent use `/dev/socket` as planned. The spike's FIFO is a test aid only.

## Risks

- **Single organizer.** Option 1 removes every WM Shell feature tied to the organizer: starting windows (splash), PiP, split screen, compat-UI restart button, back-on-root interception, unfold animations.
  - CrownOS already plans host splash placeholders. PiP must become an agent-handled window or be refused.
  - **SystemUI crash:** SystemUI re-registers and owns the tasks for about 300 ms until the agent reclaims them. During that window WM Shell decorations may flash, and leash positions are whatever the shell set.
- **F11 (patch 0015)** toggles an app to fullscreen, which is display-sized in the atlas. The compositor must swallow F11, or the agent must set FREEFORM back in `onTaskInfoChanged`.
- **Relaunch on resize.** A size change relaunches activities whose resources cross a size bucket (for example `sw600dp`). That can push single resizes past 250 ms (G3). Debouncing as the plan says helps. The latency phase measures it.
- **Non-resizable apps** cannot be MULTI_WINDOW. They need the settings in d. to stay FREEFORM, or they open fullscreen.
- **IME** is a display-level window at the bottom of the 4096x4096 display. Tasks in slots get no IME insets (`adjustResize` does nothing). This is the known v1 limitation. The HWC has to map the `InputMethod` window.
- **Toasts** are SystemUI `TYPE_TOAST` windows with no `TID:` prefix. The stock HWC drops them, so the CrownOS HWC must attach them to the focused window.
- **Client composition.** Any layer with a shadow, blur or color transform forces GPU composition of a display-sized client target. That costs memory, and the multi-window HWC drops those layers.
- **Silent overlay disable** (`mount_overlays=False`). Stale `overlay_rw` and `overlay_work`.
- **Image updates.** Reflection names and AIDL codes are pinned to this image. Re-run the `list-tasks` and `info` checks on every image update.
- **Unconfirmed until runtime:**
  - A 0-height shell caption (option 2).
  - Whether `am crash com.android.systemui` re-registering steals tasks as predicted.
  - That a `startActivityAsUser` from uid 0 with package `com.android.shell` passes AppOps.

## What the spike scripts do and expect

The spike lives in `spikes/s1-windowing/`.

**`agent/`** holds Java sources (`org.crownos.agent`) built by `agent/build.sh`.

- javac uses `-source 8` against android-35 `android.jar` plus `core-lambda-stubs.jar`.
- `stubs/` holds compile-only copies of `TaskStackListener`, `TaskOrganizer` and `WindowContainerTransactionCallback`. They are kept out of the dex.
- d8 builds `agent/build/crown-agent-spike.jar`, about 17 KB, 24 classes.
- All calls go through reflection (`Hidden`). Every event line is `t=<System.nanoTime() = CLOCK_MONOTONIC ns> <event> key=value...` and is also logged under logcat tag `CrownAgentSpike`.
- Run it with `CLASSPATH=/data/local/tmp/crown-agent-spike.jar app_process / org.crownos.agent.Spike <cmd>`.

| Command | What it does |
|---------|--------------|
| `list-tasks` | `getTasks`: id, mode, bounds, position, visible, focused, resizeable, min sizes, label, colors |
| `launch <pkg\|pkg/act> l t r b [freeform\|multiwindow]` | `startActivityAsUser` with launch bounds, windowing mode and a launch cookie. Waits for the task. |
| `resize-wct <task> l t r b [--sync]` | WCT `setBounds` through `applyTransaction`. With `--sync`, `applySyncTransaction` reports `latency_ms` until `onTransactionReady`, then applies the transaction. |
| `resize-legacy <task> l t r b` | `resizeTask` with preserve-window |
| `focus <task>`, `front <task>`, `remove <task>` | `setFocusedTask`, `moveTaskToFront`, `removeTask` |
| `info <task>` | Task record plus app label (AssetManager), manifest min size, orientation |
| `watch [s]` | `TaskStackListener` events (created, removed, front, focus, description, display, orientation, restart attempt, back on root, PiP) |
| `organize [s]` | Becomes the TaskOrganizer, places the leashes, reclaims after SystemUI restarts |
| `toast <text>` | Text toast, used to check toast placement |
| `serve <prefix>` | Root FIFO server used by the init service: reads commands from `<prefix>.in`, writes to `<prefix>.out` |

**`overlays/build.sh`** builds three code-free APKs into `overlays/build/`, signed with a throwaway key in `~/.cache/crownos-android/s1/keys`:

- the mutable SystemUI RRO,
- the static framework RRO,
- the blank HOME.

**`root-steps.sh`** (`sudo`, idempotent, `install` or `uninstall`). It never starts Waydroid.

- It refuses to run while the rootfs is mounted.
- It re-enables `mount_overlays`.
- It installs into `/var/lib/waydroid/overlay/system/`:
  - the jar to `framework/`,
  - the rc to `etc/init/`,
  - both RROs to `product/overlay/`,
  - the HOME to `product/app/CrownSpikeHome/`.
- It clears `overlay_rw` and `overlay_work`.
- It appends the host adb key to `misc/adb/adb_keys`.

**`runtime-test.sh [phase...]`** needs approval because it starts and restarts the session. Each step prints and records its expected result in `~/.cache/crownos-android/s1/runtime/<ts>/results.tsv`, together with screenshots.

| Phase | Expected results |
|-------|------------------|
| `boot` (G5) | adb as uid 2000 with `ro.adb.secure=1`; `pc` feature present; overlay files visible; `init.svc.crown_agent_spike=running` and `ro.boottime.*` set; `serve.ready uid=0`; root and shell `list-tasks` succeed; `policy_control` zeroes the bars |
| `launch` (G1) | Freeform launch in slot 0 with exact bounds; WCT resize from shell and from root, and legacy resize, all apply; focus and front move `mFocusedApp`; info shows the label "Calculator"; remove returns true; watch logs events with timestamps |
| `latency` (G3) | 12 `--sync` resizes with p95 ≤ 250 ms (PASS or FAIL). SurfaceFlinger `--latency` first frame after a non-sync resize, and the new buffer size (REVIEW) |
| `slots` (G4) | Calculator, Clock and Jelly in three non-overlapping slots all RESUMED and visible after 10 s, and still after focus moves |
| `shadows` | Default freeform window has `surfaceInsets`; none after `render_shadows_in_compositor=1`; lists layers with shadowRadius > 0 |
| `multiwindow` | Launch with mode multiwindow, visible and RESUMED; legacy caption count; no shell decor layers; leash `pos` unchanged after a bounds change; tap focus; survives `am crash com.android.systemui` |
| `ime-toast` | Settings search shows the IME. The InputMethod frame is at the display bottom, outside the slot. The toast window is TYPE_TOAST without `TID:` (REVIEW and screenshots). |
| `captions` (G2) | Legacy: `DecorCaptionView` present. `caption_on_shell=true`: legacy gone, shell caption layers present. Plus the RRO: zero caption, shadow and handles (REVIEW). Root `organize`: no `/CaptionContainer`, `/Background` or `/DecorContainer` layers, leash at the new bounds, `organizer.reclaim` after a SystemUI crash, no decor afterwards. |
| `home` | Framework RRO `[x]`; `service check wallpaper` reports not found; set-home accepted; launcher disabled; home task runs `org.crownos.spike.home`; HOME frame is tiny |
