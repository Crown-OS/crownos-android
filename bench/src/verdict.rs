use std::collections::BTreeMap;

use serde::Serialize;

use crate::budget::{Budget, Budgets};
use crate::findings::{Measurement, Metrics};
use crate::unit::Unit;

/// A run regresses when a number is this much worse than in its baseline.
pub const REGRESSION_RATIO: f64 = 0.10;

/// How a run did against one budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Within,
    OverBudget,
    Regressed,
    NotMeasured,
}

impl Status {
    pub const fn fails(self) -> bool {
        matches!(self, Self::OverBudget | Self::Regressed)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Within => "within",
            Self::OverBudget => "over budget",
            Self::Regressed => "regressed",
            Self::NotMeasured => "not measured",
        }
    }
}

/// One budget's outcome, in the budget's unit.
#[derive(Debug, Serialize)]
pub struct Verdict<'a> {
    pub id: &'a str,
    pub status: Status,
    pub unit: Unit,
    pub limit: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f64>,
    /// Why the number is missing: the run's reason, or how the plan takes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'a str>,
}

/// Judges every budget against a run's numbers and, when given, its baseline's.
pub fn judge<'a>(
    budgets: &'a Budgets,
    metrics: &Metrics,
    skipped: &'a BTreeMap<String, String>,
    baseline: Option<&Metrics>,
) -> Vec<Verdict<'a>> {
    budgets
        .iter()
        .map(|(id, budget)| {
            let reason = skipped.get(id).map(String::as_str);
            judge_one(
                id,
                budget,
                metrics.get(id),
                baseline.and_then(|b| b.get(id)),
                reason,
            )
        })
        .collect()
}

fn judge_one<'a>(
    id: &'a str,
    budget: &'a Budget,
    measured: Option<&Measurement>,
    previous: Option<&Measurement>,
    reason: Option<&'a str>,
) -> Verdict<'a> {
    let android_apps = measured.and_then(|measurement| measurement.android_apps);
    let limit = budget.limit_for(android_apps);
    let value = measured.and_then(|measurement| in_unit(measurement, budget.unit));
    let Some(value) = value else {
        return Verdict {
            id,
            status: Status::NotMeasured,
            unit: budget.unit,
            limit,
            value: None,
            baseline: None,
            note: Some(reason.unwrap_or(&budget.method)),
        };
    };
    let baseline = previous
        .filter(|previous| previous.android_apps == android_apps)
        .and_then(|previous| in_unit(previous, budget.unit));
    let status = if budget.bound.worse_by(value, limit) > 0.0 {
        Status::OverBudget
    } else if baseline.is_some_and(|baseline| regressed(budget, value, baseline)) {
        Status::Regressed
    } else {
        Status::Within
    };
    Verdict {
        id,
        status,
        unit: budget.unit,
        limit,
        value: Some(value),
        baseline,
        note: None,
    }
}

fn in_unit(measurement: &Measurement, unit: Unit) -> Option<f64> {
    measurement.unit.convert(measurement.value, unit)
}

fn regressed(budget: &Budget, value: f64, baseline: f64) -> bool {
    let worse = budget.bound.worse_by(value, baseline);
    worse > budget.noise && worse > baseline.abs() * REGRESSION_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGETS: &str = r#"
[launch.cold_first_frame]
unit = "s"
limit = 12
noise = 0.5
method = "launch with Android stopped"

[memory.android]
unit = "MiB"
limit = 1200
per_extra_app = 250
method = "payload memory.current"

[frames.scroll]
unit = "fps"
limit = 60
bound = "min"
method = "Perfetto"
"#;

    fn measured(value: f64, unit: Unit, android_apps: Option<u32>) -> Measurement {
        Measurement {
            value,
            unit,
            android_apps,
        }
    }

    fn metrics(entries: &[(&str, Measurement)]) -> Metrics {
        entries
            .iter()
            .map(|(id, measurement)| ((*id).to_owned(), *measurement))
            .collect()
    }

    fn status_of(verdicts: &[Verdict<'_>], id: &str) -> Option<Status> {
        verdicts
            .iter()
            .find(|verdict| verdict.id == id)
            .map(|verdict| verdict.status)
    }

    #[test]
    fn judges_limits_in_the_budget_unit() -> anyhow::Result<()> {
        let budgets = Budgets::parse(BUDGETS)?;
        let skipped = BTreeMap::new();
        let run = metrics(&[
            (
                "launch.cold_first_frame",
                measured(11_900.0, Unit::Milliseconds, None),
            ),
            (
                "memory.android",
                measured(1_300.0, Unit::Mebibytes, Some(1)),
            ),
            ("frames.scroll", measured(58.0, Unit::FramesPerSecond, None)),
        ]);
        let verdicts = judge(&budgets, &run, &skipped, None);
        assert_eq!(
            status_of(&verdicts, "launch.cold_first_frame"),
            Some(Status::Within)
        );
        assert_eq!(
            status_of(&verdicts, "memory.android"),
            Some(Status::OverBudget)
        );
        assert_eq!(
            status_of(&verdicts, "frames.scroll"),
            Some(Status::OverBudget)
        );
        let cold = verdicts
            .iter()
            .find(|verdict| verdict.id == "launch.cold_first_frame");
        assert_eq!(cold.and_then(|verdict| verdict.value), Some(11.9));
        Ok(())
    }

    #[test]
    fn allows_more_memory_per_extra_app() -> anyhow::Result<()> {
        let budgets = Budgets::parse(BUDGETS)?;
        let run = metrics(&[(
            "memory.android",
            measured(1_300.0, Unit::Mebibytes, Some(2)),
        )]);
        let skipped = BTreeMap::new();
        let verdicts = judge(&budgets, &run, &skipped, None);
        assert_eq!(status_of(&verdicts, "memory.android"), Some(Status::Within));
        assert_eq!(
            verdicts
                .iter()
                .find(|v| v.id == "memory.android")
                .map(|v| v.limit),
            Some(1450.0)
        );
        Ok(())
    }

    #[test]
    fn flags_regressions_past_ten_percent_and_noise() -> anyhow::Result<()> {
        let budgets = Budgets::parse(BUDGETS)?;
        let skipped = BTreeMap::new();
        let baseline = metrics(&[
            (
                "launch.cold_first_frame",
                measured(8_000.0, Unit::Milliseconds, None),
            ),
            ("memory.android", measured(900.0, Unit::Mebibytes, Some(1))),
            ("frames.scroll", measured(70.0, Unit::FramesPerSecond, None)),
        ]);
        let run = metrics(&[
            (
                "launch.cold_first_frame",
                measured(9_000.0, Unit::Milliseconds, None),
            ),
            ("memory.android", measured(980.0, Unit::Mebibytes, Some(1))),
            ("frames.scroll", measured(64.0, Unit::FramesPerSecond, None)),
        ]);
        let verdicts = judge(&budgets, &run, &skipped, Some(&baseline));
        assert_eq!(
            status_of(&verdicts, "launch.cold_first_frame"),
            Some(Status::Regressed)
        );
        assert_eq!(status_of(&verdicts, "memory.android"), Some(Status::Within));
        assert_eq!(status_of(&verdicts, "frames.scroll"), Some(Status::Within));

        let tiny = metrics(&[(
            "launch.cold_first_frame",
            measured(1_000.0, Unit::Milliseconds, None),
        )]);
        let slower = metrics(&[(
            "launch.cold_first_frame",
            measured(1_400.0, Unit::Milliseconds, None),
        )]);
        let verdicts = judge(&budgets, &slower, &skipped, Some(&tiny));
        assert_eq!(
            status_of(&verdicts, "launch.cold_first_frame"),
            Some(Status::Within),
            "0.4 s is inside the 0.5 s noise"
        );
        Ok(())
    }

    #[test]
    fn compares_memory_only_with_the_same_app_count() -> anyhow::Result<()> {
        let budgets = Budgets::parse(BUDGETS)?;
        let baseline = metrics(&[("memory.android", measured(700.0, Unit::Mebibytes, Some(1)))]);
        let run = metrics(&[(
            "memory.android",
            measured(1_100.0, Unit::Mebibytes, Some(2)),
        )]);
        let skipped = BTreeMap::new();
        let verdicts = judge(&budgets, &run, &skipped, Some(&baseline));
        let memory = verdicts
            .iter()
            .find(|verdict| verdict.id == "memory.android");
        assert_eq!(memory.map(|verdict| verdict.status), Some(Status::Within));
        assert_eq!(memory.and_then(|verdict| verdict.baseline), None);
        Ok(())
    }

    #[test]
    fn explains_what_was_not_measured() -> anyhow::Result<()> {
        let budgets = Budgets::parse(BUDGETS)?;
        let skipped = BTreeMap::from([("memory.android".to_owned(), "needs root".to_owned())]);
        let verdicts = judge(&budgets, &Metrics::new(), &skipped, None);
        let note = |id: &str| verdicts.iter().find(|v| v.id == id).and_then(|v| v.note);
        assert_eq!(note("memory.android"), Some("needs root"));
        assert_eq!(note("frames.scroll"), Some("Perfetto"));
        assert!(
            verdicts
                .iter()
                .all(|verdict| verdict.status == Status::NotMeasured)
        );
        assert!(!Status::NotMeasured.fails());
        Ok(())
    }
}
