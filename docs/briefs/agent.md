# Lane `agent`: crown-agent inside Android

## Repo and owned paths
**crownos-android**, worktree `~/crownos-lanes/agent/crownos-android`, branch `android/agent`. You own `android/` (`android/agent/`, `android/overlays/`, `android/init/`). The S1 spike code in `spikes/s1-windowing/` (22 Java files, overlay sources, init rc, build scripts) is your starting point. Read `docs/decisions/s1-windowing.md` first. Leave `spikes/` untouched; move what you keep into `android/`.

## Goal
A production root agent that makes the compositor the owner of every Android task window on the stock Android 13 image.

## Requirements

### 1. Build
- **Script:** `android/agent/build.sh` compiles with `javac` against `~/Android/Sdk/platforms/android-35/android.jar` plus compile-only stubs for hidden APIs, then dexes with `d8` (`~/Android/Sdk/build-tools/37.0.0`).
- **Output:** `~/.cache/crownos-android/agent/out/crown-agent.jar`.
- **Overlays:** the resource overlays and the blank HOME build with `android/overlays/build.sh` into `~/.cache/crownos-android/agent/out/`. Sign them with a key whose path is a parameter; there is a dev key default under `~/.cache`.

### 2. Init
- **Service:** `android/init/crown-agent.rc` runs `app_process` as root with `CLASSPATH=/system/framework/crown-agent.jar`, in class main, restarting on crash.
- **HWC socket:** the rc creates `socket crown_agent stream 0660 root system` for the HWC.

### 3. Task organizer mode (S1 option 1)
- **Registration:** register as the TaskOrganizer for every windowing mode. Re-register within one second after SystemUI dies and takes the organizer back: watch its binder death or re-check periodically.
- **Leashes:**
  - Position and crop each task leash to its atlas slot with SurfaceControl transactions. Hide no task that has a window.
  - Do not create starting windows; the host shows the splash.
- **Sizing:** use WindowContainerTransactions with `applySyncTransaction`, so `SlotApplied` is sent once the task has redrawn at its new size. Fall back to `resizeTask`.
- **Debounce:** coalesce SetSize per task to at most one transaction in flight and one pending, keeping only the latest size.
- **Atlas:** non-overlapping shelf packing on the fixed 4096x4096 display, with at least 64 px between slots. Repack only the moved task.
- **Overflow:** if tasks do not fit, allow an overlap and keep the most recently focused task on top.
- **Code layout:** the packer is a pure, host-testable class.

### 4. Agent-wire v1 on both sockets
- **Codec:** a pure-Java codec tested on the host JVM against `contracts/agent-wire-vectors.txt`. Provide a test runner script, no JUnit dependency needed.
- **HWC socket** (`/dev/socket/crown_agent`, inherited from init through `ANDROID_SOCKET_crown_agent`):
  - Handle `Hello`, `SetSize`, `Focus` (setFocusedTask), `Close` (remove the task), `Back` (inject KEYCODE_BACK into that task's display, or dispatch it to the task) and `Visibility` (record it; a hidden task's leash may be hidden).
  - Send `TaskInfo` on appear and on change, plus `SlotApplied`, `TaskRemoved` and `Activate`.
- **Daemon socket:**
  - The agent creates `/data/crownos/agent.sock` with mode 0600, owned by the host user's uid. Find how Waydroid exposes the host uid (`s1-windowing.md`, the vendor patch "init: Define host user") and record it in your report.
  - It checks SO_PEERCRED and accepts that uid only.
  - Handle `Hello`, `Launch` (resolve the launcher activity and start it FREEFORM with bounds in a free slot; reply `TaskOpened` or `LaunchFailed`), `ListTasks`, `QueryKeepAlive`, `QueryGsfId`, `Shutdown`, `OpenUri` (VIEW intents only, schemes `market` and `https`), and `InstallPackage` (PackageInstaller session, only for files directly under `/data/crownos/inbox/`, no path traversal). Send `InstallResult`.
  - Send `Ready{boot_ms}` once `sys.boot_completed` is set and the organizer is registered.
  - Push `TaskOpened` and `TaskClosed` as they happen.
- **Keep-alive bits:**
  - AUDIO: an active AudioPlaybackConfiguration.
  - CALL: telephony or VoIP call state.
  - INSTALL: active PackageInstaller sessions.
  - DOWNLOAD: running DownloadManager downloads.
  - FOREGROUND_SERVICE: a foreground service with an ongoing notification.
- **GSF id:** read the Google services framework id (gservices `android_id`) as root. Reply with an empty string when it is not available.

### 5. Boot-time settings
Apply once the system is ready:
- `policy_control` = `immersive.full=*` (Waydroid patch 0029 key);
- `render_shadows_in_compositor` = 1;
- `waydroid.active_apps` = `crownos`;
- disable `com.android.launcher3` and set the blank HOME;
- enable non-resizable apps in multi-window;
- keep the screen on, since the daemon owns the lifecycle.

`persist.crownos.safe_mode=1` disables organizer mode and every boot-time change except `active_apps`.

### 6. Overlays
- **Framework overlay:** keep the no-wallpaper framework overlay.
- **SystemUI overlay:** keep the SystemUI no-decor overlay as the fallback when organizer mode is off.
- **Blank HOME:** keep the code-free blank HOME APK.

## Acceptance
- **Builds:** `build.sh` and `overlays/build.sh` succeed through build-slot. The jar and APKs are produced.
- **Host tests:** codec vectors, the atlas packer (no overlap, the gap is honoured, overflow ordering), command validation (OpenUri schemes, inbox path checks) and the SetSize coalescing logic. All must pass.
- **Hidden APIs:** every hidden API call is behind reflection with a clear failure log, so missing methods never crash the agent.
- **Commits:** on `android/agent`.
