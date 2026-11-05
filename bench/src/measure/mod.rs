//! One module per measurement, ported from S3's `spikes/s3-platform/measure/`.

pub mod boot;
pub mod cgroup_memory;
pub mod idle;
pub mod launch;
pub mod leftovers;
pub mod lifecycle;
pub mod mem_available;
pub mod pss;

use crate::findings::MetricId;

/// The client of `crown-androidd` that launches and stops Android.
pub const CLI: &str = "crown-android";
pub const NOT_RUNNING: &str = "Android is not running";
pub const NEEDS_ROOT: &str = "needs root";

/// Every budgeted number the bench takes.
pub const METRICS: [MetricId; 14] = [
    launch::SPLASH,
    launch::COLD_FIRST_FRAME,
    launch::WARM_FIRST_FRAME,
    cgroup_memory::ANDROID,
    pss::HWC,
    pss::AGENT,
    pss::DAEMON,
    idle::ANDROID_CPU,
    idle::ANDROID_WAKEUPS,
    idle::DAEMON_CPU,
    lifecycle::STOP,
    leftovers::PROCESSES,
    leftovers::MOUNTS,
    mem_available::LOST,
];
