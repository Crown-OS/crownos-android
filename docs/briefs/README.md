# Wave 2 lanes: shared rules

Every lane agent reads this file and then its own brief, `docs/briefs/<lane>.md`.

## What we are building

CrownOS (Arch-based, Rust desktop, Smithay compositor `crownpositor`) runs Android apps through ONE Waydroid session. Every Android task is a native, compositor-owned window: it opens, moves, resizes, closes, tiles, snaps and alt-tabs like any Wayland app.

Fixed decisions:
- **Android base:** Android 13 on official Waydroid LineageOS 20 images. No custom-built images. Android-side changes ship as Waydroid overlay files (`/var/lib/waydroid/overlay`), props and scripts.
- **Install flow:** an optional "Android apps" switch in the installer (with Google Play or without). Waydroid packages install offline at install time. A root oneshot service downloads and sets up the images after first boot, and a crownotify notification with a loader shows progress.
- **Lifecycle:** the runtime stops completely after 5 minutes without an Android window and starts on demand.
- **NVIDIA:** NVIDIA-only machines get Venus acceleration.

Read before you start:
- **Plan:** `/home/marvelxcodes/.claude/plans/create-complete-production-grade-transient-sunbeam.md`. The architecture section still says the host stacking order is mirrored; the atlas design below replaces that.
- **Contracts:** `/e/CrownOS/crownos-android/contracts/README.md` and `agent-wire.md`, plus the crates `agent-wire`, `android-protocol`, `setup-state`, `bundle` and `venus` in `/e/CrownOS/crownos-android/crates/`.
- **Decision records:** `/e/CrownOS/crownos-android/docs/decisions/`:
  - `s1-windowing.md` (Android task control, caption removal)
  - `s2-hwc-build.md` (HWC build)
  - `s3-platform.md` (kernel, fences, measurement)
  - `s4-venus.md`
  - `d1-kernel.md`

## Architecture in one paragraph

**Pixels.** Android runs every task in FREEFORM mode on one fixed 4096x4096 display. The `crown-agent` (Java, run as root by an overlay init.rc through `app_process`) registers as the TaskOrganizer. It packs tasks into non-overlapping atlas slots, positions and crops their leashes itself, and sizes them with WindowContainerTransactions (`applySyncTransaction`). The CrownOS hwcomposer fork (`hwcomposer.waydroid.so`, built out of tree with the NDK) shows each task as its own `xdg_toplevel`, sized to the task's slot. Each Android layer becomes a subsurface at `displayFrame − slot origin`, and input is shifted back by the slot origin. Moving a window on the host never touches Android.

**Messages.**
- The HWC talks to the agent over `/dev/socket/crown_agent`, using the agent-wire records `SetSize`/`SlotApplied` etc.
- The daemon `crown-androidd` talks to the agent over `/data/crownos/agent.sock`, using the agent-wire records `Launch`/`Tasks`/`KeepAlive`/...
- The compositor gets Android extras through `crownos-android-shell-v1`: task id, Back, resize lock, title color, placeholder handoff. Everything else uses standard xdg-shell.

## Your workspace

- **Lane directory:** `~/crownos-lanes/<lane>/`. It mirrors `/e/CrownOS`. The repos your lane edits are git worktrees on branch `android/<lane>`. Every other entry is a symlink that you may READ but must NEVER write.
- **Never touch `/e/CrownOS`.** Do not edit, build in or run git commands against anything under it, except reading. The user has uncommitted work there.
- **Contracts are read-only.** Do not change any contract file or contract crate (`crates/agent-wire`, `android-protocol`, `setup-state`, `bundle`, the venus signatures, `contracts/`). If a contract blocks you, finish what you can and report the exact problem in your final message.
- **Stay inside your owned paths.** Other lanes run at the same time in their own worktrees and own their own paths.

## Building

- **Use the build slot.** Run every build and test through `~/crownos-lanes/bin/build-slot <command>`, for example `~/crownos-lanes/bin/build-slot cargo test`. It serializes builds across all lanes, because this machine has 14 GB of RAM and little of it free. Keep builds small: debug profile, no `--release` unless the brief asks, and the `-p <crate>` scope where you can.
- **Set your own target dir.** Use `CARGO_TARGET_DIR=$HOME/.cache/crownos-android/target/<lane>-<repo>`. `/e` is almost full; never build into it.
- **Keep files out of RAM.** Never write big files to `/tmp`, which is RAM-backed. Use `~/.cache/crownos-android/tmp/`.
- **Rust gate,** in every Rust repo you touch: `cargo fmt -- --check && cargo clippy --all-targets -- -D warnings && cargo test`, with the features the repo's CI uses.

## Testing scope

The user tests on real hardware later. So:
- **Never run Android:** do NOT start Waydroid or Android, a nested compositor, a VM, or a GPU render server, and do NOT use sudo.
- **Do write tests:** unit tests and integration tests against fakes (socket pairs, temp dirs, fake sysfs trees, in-process Wayland servers, mock D-Bus) are required, and they must pass.
- **Where runtime matters:** write the code so the later hardware test can exercise it, and list what only hardware can prove.

## Code rules (from the user's CLAUDE.md)

- **Naming and size:** meaningful, self-explanatory names. Idiomatic Rust. Simple solutions, the best performance in the fewest lines, a low memory footprint, hardware acceleration where it applies.
- **Comments:** no useless comments. If code needs a comment to be understood, restructure it. Doc comments on public items are welcome.
- **Modules:** split into focused files and modules, never one big file. Utilities go in `src/util/<name>.rs`.
- **Ownership:** borrow instead of cloning, use `Cow<str>` when ownership is conditional, and keep lifetime annotations to a minimum.
- **Errors:** no `unwrap`/`expect` outside tests. `thiserror` for library error types, `anyhow` for binaries.
- **Types:** make invalid states unrepresentable. Use newtypes and enums instead of bare integers and boolean parameters. No magic numbers: use named constants. Return early instead of nesting.
- **Existing repos:** follow their structure and style. Read neighbouring code first and reuse what exists.
- **C++, Java and shell code:** follow the same spirit: small files, clear names, no dead code.

## Commits

- **Where and when:** commit in your worktree on `android/<lane>` at each milestone, with messages in the style of the repo's history (imperative subject, a body that says why).
- **No attribution:** never add co-author lines or AI attribution.
- **Never** push, rebase or rewrite published history.
- **Commit only your lane's files.**

## Done means

1. Every requirement in your brief is implemented, with no placeholders, TODOs or stubs left in your owned paths.
2. The gates are green in every repo you touched.
3. Your final message, under 900 words, covers:
   - what you built, file by file at module level;
   - test results;
   - the commits you made, by hash;
   - what only a hardware test can prove;
   - anything other lanes or the lead must do.
