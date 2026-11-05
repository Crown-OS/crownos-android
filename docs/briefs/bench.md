# Lane `bench`: budgets harness and process-monitor identity

## Repos and owned paths
- **crownos-android**, worktree `~/crownos-lanes/bench/crownos-android`, branch `android/bench`. You own `bench/`: the `crown-android-bench` binary and `bench/budgets/`.
- **crownos-process-monitor**, worktree `~/crownos-lanes/bench/crownos-process-monitor`, branch `android/bench`. You own the changes needed for Waydroid identity.

## Goal
1. One command that measures the Android runtime against the plan's budgets and fails on regressions.
2. The process monitor shows Android as one app.

## Requirements
1. **`bench/budgets/budgets.toml`:** the budgets table from the plan (launch, resize, memory, idle, idle stop, frames, setup), each with a unit and a limit.
2. **`crown-android-bench`:** port the S3 measurement scripts (`spikes/s3-platform/measure/*.py`) into Rust modules, one module per measurement:
   - boot time;
   - container cgroup memory (`memory.current` and `memory.stat` of the LXC payload cgroup; find the path the way `measure/memory.py` does);
   - per-process PSS from `smaps_rollup`;
   - idle CPU and wakeups from cgroup `cpu.stat`;
   - lifecycle timings: freeze/unfreeze, stop, start;
   - the MemAvailable delta after idle stop;
   - leftover processes and mounts after stop;
   - launch timings, by calling `crown-android launch` and watching the app_id appear through `ext-foreign-toplevel-list`.

   Each run writes a JSON result to `~/.cache/crownos-android/bench/<time>.json`, compares it with the budgets and with an optional baseline JSON, and exits non-zero on a budget miss or a regression over 10%. Root-only measurements are skipped with a note when not root.
   - Pure parsing (smaps_rollup, memory.stat, cpu.stat, budgets comparison) has unit tests with fixture files.
   - Never start Android in tests.
3. **Process-monitor identity:** recognise the Waydroid LXC cgroup (`lxc.payload.waydroid` and the lxc monitor scope) as `CgroupIdentity::Container` (`crates/monitor-system/src/apps/cgroup.rs`), so Android shows up as one app named "Android". If the existing types need it, add a dedicated variant or a label; keep the change minimal and follow the crate's conventions (`docs/conventions.md`). Add tests with cgroup path fixtures.

## Acceptance
- **Gates:** green in both workspaces, and `crown-android-bench --help` works.
- **Commits:** on `android/bench` in both worktrees.
