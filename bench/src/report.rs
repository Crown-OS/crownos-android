use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::findings::Findings;
use crate::host::Privilege;
use crate::unit::Unit;
use crate::verdict::Verdict;

/// Format version of a result file.
pub const FORMAT: u32 = 1;
const RESULTS_DIRECTORY: &str = ".cache/crownos-android/bench";
const STATUS_WIDTH: usize = 14;
const ID_WIDTH: usize = 32;

/// A run's result file. Its `metrics` are what a later run reads as its baseline.
#[derive(Serialize)]
pub struct RunReport<'a> {
    pub format: u32,
    pub started: String,
    pub kernel: String,
    pub privilege: Privilege,
    pub apps: &'a [String],
    pub baseline: Option<&'a Path>,
    pub verdicts: &'a [Verdict<'a>],
    #[serde(flatten)]
    pub findings: &'a Findings,
}

/// `~/.cache/crownos-android/bench`.
pub fn default_directory() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set; pass --out")?;
    Ok(PathBuf::from(home).join(RESULTS_DIRECTORY))
}

/// Writes `<directory>/<stem>.json` and returns its path.
pub fn write(directory: &Path, stem: &str, report: &RunReport<'_>) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("could not create {}", directory.display()))?;
    let path = directory.join(format!("{stem}.json"));
    let json = serde_json::to_vec_pretty(report)?;
    std::fs::write(&path, json).with_context(|| format!("could not write {}", path.display()))?;
    Ok(path)
}

/// The findings of an earlier result file.
pub fn load(path: &Path) -> Result<Findings> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not a bench result", path.display()))
}

/// One line per budget, then one per failed step.
pub fn print_summary(
    verdicts: &[Verdict<'_>],
    failures: &BTreeMap<String, String>,
    out: &mut impl Write,
) -> io::Result<()> {
    for verdict in verdicts {
        write!(
            out,
            "{:<STATUS_WIDTH$}{:<ID_WIDTH$}",
            verdict.status.label(),
            verdict.id
        )?;
        match verdict.value {
            Some(value) => {
                let limit = quantity(verdict.limit, verdict.unit);
                write!(out, "{} (limit {limit}", quantity(value, verdict.unit))?;
                if let Some(baseline) = verdict.baseline {
                    write!(out, ", baseline {}", quantity(baseline, verdict.unit))?;
                }
                write!(out, ")")?;
            }
            None => write!(out, "{}", verdict.note.unwrap_or_default())?,
        }
        writeln!(out)?;
    }
    for (step, error) in failures {
        writeln!(out, "{:<STATUS_WIDTH$}{step}: {error}", "failed")?;
    }
    Ok(())
}

fn quantity(value: f64, unit: Unit) -> String {
    match unit.symbol() {
        "" => value.to_string(),
        "%" => format!("{value}%"),
        symbol => format!("{value} {symbol}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Budgets;
    use crate::verdict::judge;

    #[test]
    fn writes_a_result_a_later_run_reads_back() -> Result<()> {
        let budgets = Budgets::load(None)?;
        let baseline = load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/baseline.json"
        )))?;
        let mut findings = Findings {
            metrics: baseline.metrics.clone(),
            ..Findings::default()
        };
        findings.fail(
            "stop",
            "Waydroid processes or mounts were left after the stop",
        );
        let verdicts = judge(
            &budgets,
            &findings.metrics,
            &findings.skipped,
            Some(&baseline.metrics),
        );
        let apps = ["com.android.settings".to_owned()];
        let report = RunReport {
            format: FORMAT,
            started: "2026-11-05T09:34:00Z".to_owned(),
            kernel: "7.2.9".to_owned(),
            privilege: Privilege::User,
            apps: &apps,
            baseline: None,
            verdicts: &verdicts,
            findings: &findings,
        };
        let directory =
            std::env::temp_dir().join(format!("crown-android-bench-report-{}", std::process::id()));
        let path = write(&directory, "20261105T093400Z", &report)?;
        let read_back = load(&path)?;
        std::fs::remove_dir_all(&directory)?;
        assert_eq!(read_back.metrics, baseline.metrics);
        assert_eq!(read_back.failures.len(), 1);

        let mut summary = Vec::new();
        print_summary(&verdicts, &findings.failures, &mut summary)?;
        let summary = String::from_utf8(summary)?;
        assert!(summary.contains(
            "within        launch.splash                   84.2 ms (limit 100 ms, baseline 84.2 ms)"
        ));
        assert!(
            summary.contains(
                "not measured  frames.scroll                   frame rate while scrolling"
            )
        );
        assert!(summary.contains("failed        stop: Waydroid processes"));
        Ok(())
    }
}
