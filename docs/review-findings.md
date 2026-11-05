# Wave 5 review findings

Verified by the reviewers against the code, read-only. Every finding lists its repo, where it is, what breaks, and the fix. Fix agents work from this file; each finding is marked when its fix lands.

## Security review

- **C1 (Critical), crownos-android packaging.** `channels.cfg` let any local user drive an unauthenticated root `waydroid init` and pick the image root installs. **Fixed** in `57c0247`.

| Id | Sev | Repo | Where | Problem | Fix |
|---|---|---|---|---|---|
| S-M1 | Medium | crownos-android agent | `Agent.java:100-101`, `system/HostUser.java:39-49`, `system/PackageInstall.java:35,49,133`, `link/DaemonSession.java:105-115` | The host user owns `/data/crownos` and `inbox`, and owns all of `/data` when the host uid is 1000. `O_NOFOLLOW` covers only the last component, so a swapped symlink makes the root agent open, unlink or chmod arbitrary paths. `ownDirectory`'s lstat → lchown → `Os.chmod` can race. A FIFO hangs the install thread. | Walk by descriptor: open `/data`, then each component through `/proc/self/fd/N/<c>` with `O_DIRECTORY\|O_NOFOLLOW`, and fstat that it is a directory owned by the host uid. Open and unlink the APK through the inbox fd with `O_NOFOLLOW\|O_NONBLOCK` and require S_ISREG. In `ownDirectory`, use fchown/fchmod on an `O_NOFOLLOW` fd. |
| S-L1 | Low | crownpositor | `utils/client_identity.rs` (`KnownExecutable::of_path`, `executable_of`) | The daemon and crownotify are recognised by executable file name, read from `/proc/<pid>/exe` after accept. A same-uid copy in `$HOME` passes, though no boundary is crossed today. | Take a pidfd (`SO_PEERPIDFD`) and compare the exe's dev and inode with `/usr/bin/crown-androidd` or `/usr/bin/crownotify`. Allow dev builds only through a config override. |
| S-L2 | Low | crownos-android agent and daemon | `Agent.java:47`, `PeerLink.java` adopt, `crates/runtime/src/agent.rs:33-35` | The container maps no uids, so Android's system uid (1000) and radio uid (1001) match host uids: those platform processes can reach `agent.sock`, evict the daemon and send commands. The daemon never checks that the listener is uid 0. The HWC socket allows the same eviction. | Agent: admit only peers whose `/proc/<pid>/ns/mnt` differs from its own, and refuse a new peer while one is handshaken. Daemon: require SO_PEERCRED uid 0 after connecting. |
| S-L3 | Low | crownos-android cli | `cli/src/inbox.rs:30` | `fs::copy` to a predictable name follows a planted symlink. | `OpenOptions::create_new(true)` with `custom_flags(O_NOFOLLOW)`, then `io::copy`. |
| S-L4 | Low | crownos-android setup | `org.crownos.android.policy`, `setup/src/main.rs:30-48` | The pkexec action allows every subcommand. `make-manifest --root` writes as root, and `run` executes outside the sandbox. auth_admin is still required. | When `PKEXEC_UID` is set, accept only `remove [--wipe-data]` and `enable`. |
| S-L5 | Low | crownos-waydroid-hwc | `task/geometry.h:24-26`, `task/registry.cpp:15`, `agent/bridge.cpp` SlotApplied, `agent/link.cpp:54-69` | int32 slot arithmetic can overflow, and the receive buffer has no cap. Only the root agent can send these, so this is hardening. | Reject slots outside [0, display]. Compute in int64. Cap `received`. |

Inherited, not fixed here: the binder nodes are 0666, as in stock Waydroid. Track upstream.

## Correctness: HWC and existing-repo diffs

| Id | Sev | Repo | Where | Problem | Fix |
|---|---|---|---|---|---|
| R4-M1 | Medium | crownpositor | `layout/aspect.rs:20-33` `Driver::of`, called at `shell/mod.rs:1714` | Corner resizes of an aspect-locked window flip between two sizes on every motion event. The driver compares the cumulative request with the previous motion's result. This sends a resize to Android on every event. | Pick the driver from the grab's initial content size, or from `requested` alone (`w*ratio.h >= h*ratio.w`). |
| R4-M2 | Medium | crownpositor | `shell/workspace.rs:309-324` `replace_tile`, `shell/tile.rs:507-519` `take_place_of`, also `tile.rs:451` `set_android` | A size-locked window that replaces a tiled splash, or gets `fixed` after mapping, becomes Floating but stays among the tiled entries of `tiles`. It then draws and hit-tests below other floating windows and never raises. | When a replaced or locked tile now floats, move it to the end of `tiles`, as `push_tile` does. |
| R4-M3 | Medium | crownos-android daemon (and crownotify) | daemon `actions.rs:23`, `notify/mod.rs:94-98`; crownotify `store.rs:145-158`, `daemon/state.rs:153-161` | When the user swipes the setup loader away, crownotify sends `Closed(Dismissed)`. The daemon forgets the id, so the next update posts a new loader and the old one stays frozen in the center. | Daemon: forget the id only for the Expired or Closed-by-call reasons, and keep updating the same id after Dismissed. |
| R4-M4 | Medium | crownos-settings | `net/android/worker.rs:30,41,49`, `net/android/daemon.rs:86` | After "Stop Android now", the page keeps showing Running. Events that arrive during a call are queued in the client and only forwarded after the next socket read. | Forward queued events (non-blocking) after every command and call. |
| R4-M5 | Medium | crownos-settings | `net/android/survey.rs:13` `IMAGES_DIR` | Storage always says "Not downloaded yet". Images live in `/var/lib/crownos/android/images`, not `/var/lib/waydroid/images`. | Measure `/var/lib/crownos/android/images`, plus `staging/`. |
| R4-M6 | Medium | crownos-waydroid-hwc | `hwcomposer/modes/task-window.cpp:342` | Toast subsurfaces accept input. A tap on a toast maps to the atlas bottom and lands in another task. | Accept input only for `layer_role::input_method`. |
| R4-L1 | Low | crownos-waydroid-hwc | `modes/task-window.cpp:368-376` | A failed `sync_merge` drops framebuffer release fences, so SurfaceFlinger can reuse a buffer the compositor still reads. | On failure, keep `merged`, close `release`, and CPU-wait on the dropped fence. |
| R4-L2 | Low | crownos-waydroid-hwc | `fence/frame_sync.cpp:58-83` | vgem fences auto-signal after 10 s, so the release fence of a frame shown longer can lie. | Refresh the vgem fence for a static frame before about 9 s, or document it. |
| R4-L3 | Low | crownpositor | `input/mouse/mod.rs:215,255-271` | A stale `side_button_back` flag after an early return swallows a later side-button release, and the implicit grab sticks. | Handle the flag at the top of `on_pointer_button`, before the early returns. |
| R4-L4 | Low | crownpositor | `handlers/crownos_android_shell.rs:89-101` | The placeholder mark is consumed even when the splash hasn't mapped, so a late splash never gets `close`. | Look up the splash window first, and re-arm the mark if it isn't mapped. |
| R4-L5 | Low | crownotify | `store.rs:144-160`, `allocate_id` `:520`, `ipc/server.rs:310` | An unknown nonzero `replaces_id` is used, but the id counter never moves past it, so later notifications can collide. | `fetch_max` the counter when an unknown id is filed. |
| R4-L6 | Low | crownotify | `ui/activation.rs:33-36` | A `desktop-entry` ending in `.desktop` raises nothing. | Trim `.desktop` before raising. |
| R4-L7 | Low | crownos-process-monitor | `crates/monitor-system/src/actions/app.rs:33-42` | The End/Pause Android confirmation counts only the payload cgroup's own procs (about 1). | Count across the cgroup subtree. |
| R4-L8 | Low | crownos-settings | `net/android/daemon.rs:123-124` | A transport error during Stop or the GSF query drops the client without arming the retry. | Call `self.unreachable()`. |
| R4-L9 | Low | crownos-settings | `util/tree_size.rs:10-18` | `tree_size` holds a `ReadDir` handle for every pending directory, which can exhaust fds on a wide tree. | Stack `PathBuf`s and open each directory when it is popped. |

## Correctness: Rust and Java

Pending.

## Cross-component consistency

Pending.
