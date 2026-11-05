# CrownOS Android apps: task list

Plan: `~/.claude/plans/create-complete-production-grade-transient-sunbeam.md`

Status keys: `[ ]` open, `[~]` in progress, `[x]` done, `[!]` blocked (reason noted).

Ground rules for every agent:
- No agent starts Waydroid, a nested compositor or a VM without the user's approval. Runtime steps go into `spikes/<id>/runtime-test.sh`.
- No agent uses sudo. Root steps go into one reviewed script, `spikes/<id>/root-steps.sh`, which the user runs.
- Large build output goes in `~/.cache/crownos-android/<id>/`, not on `/e`, which has about 20 GB free.
- Builds run with at most `-j4`.

## Wave 0: spikes

### Offline half
- [~] S1 Android windowing: static analysis of the pinned image, plus the agent prototype and the experiment kit.
- [x] S2 HWC build: NDK out-of-tree build of the stock HWC, then symbol resolution.
  - H1, H2 and H3 GO: 290/290 imports resolve as `std::__1`, the HIDL ABI matches the image, and the gralloc handle layout matches. H4 NEEDS-RUNTIME.
  - Cold build 1 min 42 s, cache 127 MB. No LineageOS tree needed.
  - Details: `docs/decisions/s2-hwc-build.md`. Build: `crownos-waydroid-hwc/ndk/`.
- [x] S3 Platform: linux-cachyos kernel config check, plus the measurement scripts.
  - K1 GO: linux-cachyos 7.2 and 7.3 have the C binder built in.
  - K2 GO: no kernel has SW_SYNC. The HWC fork uses drm_syncobj for acquire fences and vgem for release fences.
  - K3 GO: measurement scripts ready.
  - DECISION DUE 2026-12-15: upstream drops the C binder in 7.4. Options are linux-cachyos-lts 6.18, or a CrownOS-built kernel without LTO using the Rust binder.
  - Details: `docs/decisions/s3-platform.md`.
- [x] S4 Venus: chain analysis, the Mesa/ANGLE Android build, and the host render server build.
  - V1 GO, V2 GO, V4 GO but we would own about 3,000 lines of out-of-tree Mesa and virglrenderer patches, V3 NEEDS-RUNTIME.
  - Details: `docs/decisions/s4-venus.md`.

### Runtime half (approved: nested crownpositor, zen kernel, runbook `spikes/run-wave0.sh`)
- [ ] S1 runtime experiments (needs the user to run the runbook)
- [ ] S2 overlay load test (needs the user to run the runbook)
- [ ] S3 baseline measurements (needs the user to run the runbook). The QEMU snapshot is deferred to E2.
- [ ] S4 full-chain test on the RTX 4050. Deferred to E1, because it needs MUX dGPU mode. `root-steps.sh revert` added.

### Decisions
- [x] D1: kernel after 7.3 is linux-cachyos-lts 6.18 (`docs/decisions/d1-kernel.md`).

### Decision gate
- [ ] Go/no-go review of S1 and S2, and the fallbacks chosen.

## Wave 1: lead
- [x] Freeze contracts 1 to 13 in `contracts/`.
  - Contracts 4, 5, 6, 7, 8, 9 and 10 are frozen. Crates `android-protocol`, `setup-state` and `bundle` hold the golden tests, and fmt, clippy and test are green.
  - Contracts 1, 2, 3, 11, 12 and 13 frozen after the offline spikes (the user deferred runtime tests).
- [x] Scaffold the workspace, plus the `android/base` branches of crownos-protocols (protocol + macro) and crownos-config (android section, android.ron).
- [x] Fakes: each lane builds the ones it needs (see its brief).
- [x] Lane briefs in `docs/briefs/`, worktrees under `~/crownos-lanes/<lane>` (`tools/lane.sh`), builds serialized by `~/crownos-lanes/bin/build-slot`.

## Wave 2: lanes
- [~] L1 compositor protocol server and trust check
- [~] L2 compositor window behaviour
- [~] L3 HWC fork
- [~] L4 agent
- [~] L5 crown-androidd and crown-android
- [~] L6 setup service and bundle tooling
- [x] L7 crownotify (crownui unchanged): commits dd2b4e8, 75a3385, a05209b, 22b4445 on android/notify; 364 tests
- [x] L8 installer and settings: installer 2991783, 1fe3c44; settings 8e7a2a9 on android/installer; 311 + 178 tests
- [x] L9 ISO and pkgbuilds: crownos-android 2eb491f, crownos-pkgbuilds 3f68ffe, crownos-iso 9a4453b..4d40021 on android/iso
- [~] L10 Venus tier

## Wave 3: integration
- [ ] E1 hardware end-to-end
- [ ] E2 ISO and QEMU install test
- [~] E3 bench harness and process-monitor identity (runs as Wave 2 lane `bench`)

## Wave 4: optimization
- [ ] P1 Android memory and boot
- [ ] P2 frame path
- [ ] P3 host side

## Wave 5: review
- [ ] Code review
- [ ] Security review
- [ ] Release checklist
