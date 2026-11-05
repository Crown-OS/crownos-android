use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

/// Apps of every LineageOS 20 image: Settings, launched cold, then Files, launched warm.
const DEFAULT_APPS: [&str; 2] = ["com.android.settings", "com.android.documentsui"];

const ABOUT: &str = "Measures the CrownOS Android runtime against its budgets";
const LONG_ABOUT: &str = "\
Measures the CrownOS Android runtime against its budgets.

A run stops Android, launches the apps through `crown-android launch` (the first while \
Android is stopped, the others while it runs) and times the boot, the splash and each \
window through ext-foreign-toplevel-list. It then measures the container's cgroup memory, \
PSS and idle CPU and wakeups, freezes and unfreezes the container, stops it through \
`crown-android stop`, counts what the stop left and the MemAvailable it lost, and starts \
Android once more.

The result goes to ~/.cache/crownos-android/bench/<time>.json. The run exits non-zero \
when a step fails, a number misses its budget, or a number is more than 10% worse than \
in --baseline. Android's own PSS and perf wakeups need root and are skipped otherwise.";

#[derive(Debug, Parser)]
#[command(name = "crown-android-bench", version, about = ABOUT, long_about = LONG_ABOUT)]
pub struct Args {
    /// Android package to launch; repeat for more. The first launch starts Android.
    #[arg(long = "app", value_name = "PACKAGE", default_values = DEFAULT_APPS)]
    pub apps: Vec<String>,
    /// An earlier result to compare with.
    #[arg(long, value_name = "JSON")]
    pub baseline: Option<PathBuf>,
    /// Budgets to judge by instead of the built-in budgets.toml.
    #[arg(long, value_name = "TOML")]
    pub budgets: Option<PathBuf>,
    /// Judge an existing result instead of measuring, as a CI check does.
    #[arg(long, value_name = "JSON")]
    pub compare: Option<PathBuf>,
    /// Where results go [default: ~/.cache/crownos-android/bench].
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Seconds Android settles after the launches before memory and idle use are measured.
    #[arg(long, value_name = "SECONDS", default_value_t = 30)]
    pub settle: u64,
    /// Seconds of idle sampling.
    #[arg(long, value_name = "SECONDS", default_value_t = 60)]
    pub idle: u64,
    /// Seconds Android stays frozen.
    #[arg(long, value_name = "SECONDS", default_value_t = 5)]
    pub frozen_hold: u64,
    /// Seconds between a stop and the MemAvailable reading after it.
    #[arg(long, value_name = "SECONDS", default_value_t = 10)]
    pub after_stop_settle: u64,
    /// Seconds to wait for a boot, a window or a stop.
    #[arg(long, value_name = "SECONDS", default_value_t = 120)]
    pub timeout: u64,
}

impl Args {
    pub fn settle(&self) -> Duration {
        Duration::from_secs(self.settle)
    }

    pub fn idle(&self) -> Duration {
        Duration::from_secs(self.idle)
    }

    pub fn frozen_hold(&self) -> Duration {
        Duration::from_secs(self.frozen_hold)
    }

    pub fn after_stop_settle(&self) -> Duration {
        Duration::from_secs(self.after_stop_settle)
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout)
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn parses_apps_and_defaults() -> Result<(), clap::Error> {
        Args::command().debug_assert();
        let defaults = Args::try_parse_from(["crown-android-bench"])?;
        assert_eq!(defaults.apps, DEFAULT_APPS);
        assert_eq!(defaults.idle(), Duration::from_secs(60));
        let chosen = Args::try_parse_from([
            "crown-android-bench",
            "--app",
            "org.lineageos.jelly",
            "--app",
            "com.android.settings",
            "--baseline",
            "base.json",
        ])?;
        assert_eq!(chosen.apps, ["org.lineageos.jelly", "com.android.settings"]);
        assert_eq!(chosen.baseline, Some(PathBuf::from("base.json")));
        Ok(())
    }
}
