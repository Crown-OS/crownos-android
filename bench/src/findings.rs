use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::unit::Unit;

/// A budgeted number the bench takes, and the unit it records it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricId {
    pub name: &'static str,
    pub unit: Unit,
}

/// One number a run took.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub value: f64,
    pub unit: Unit,
    /// Android apps with a window open when it was taken, for budgets that grow per app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_apps: Option<u32>,
}

/// Measurements by metric name.
pub type Metrics = BTreeMap<String, Measurement>;

/// What a step's result adds to a run besides its JSON.
pub trait Report: Serialize {
    /// Records the step's budgeted numbers, why any is missing, and what went wrong.
    fn report(&self, _step: &str, _findings: &mut Findings) {}
}

/// Everything a run found, step by step.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Findings {
    pub metrics: Metrics,
    /// Why a budgeted number was not taken, by metric name.
    #[serde(default)]
    pub skipped: BTreeMap<String, String>,
    /// What went wrong, by step.
    #[serde(default)]
    pub failures: BTreeMap<String, String>,
    /// What each step saw.
    #[serde(default)]
    pub steps: BTreeMap<String, serde_json::Value>,
}

impl Findings {
    /// Keeps a step's result, or its error as a failure.
    pub fn record<T: Report>(&mut self, step: &str, outcome: anyhow::Result<T>) -> Option<T> {
        let result = match outcome {
            Ok(result) => result,
            Err(error) => {
                self.fail(step, format!("{error:#}"));
                return None;
            }
        };
        result.report(step, self);
        match serde_json::to_value(&result) {
            Ok(json) => {
                self.steps.insert(step.to_owned(), json);
            }
            Err(error) => self.fail(step, error.to_string()),
        }
        Some(result)
    }

    pub fn metric(&mut self, metric: MetricId, value: f64) {
        self.metric_with_apps(metric, value, None);
    }

    pub fn metric_with_apps(&mut self, metric: MetricId, value: f64, android_apps: Option<u32>) {
        let measurement = Measurement {
            value,
            unit: metric.unit,
            android_apps,
        };
        self.metrics.insert(metric.name.to_owned(), measurement);
    }

    pub fn skip(&mut self, metric: MetricId, reason: impl Into<String>) {
        self.skipped.insert(metric.name.to_owned(), reason.into());
    }

    pub fn fail(&mut self, step: &str, error: impl Into<String>) {
        self.failures.insert(step.to_owned(), error.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPLASH: MetricId = MetricId {
        name: "launch.splash",
        unit: Unit::Milliseconds,
    };

    #[derive(Serialize)]
    struct Probe(f64);

    impl Report for Probe {
        fn report(&self, _step: &str, findings: &mut Findings) {
            findings.metric(SPLASH, self.0);
        }
    }

    #[test]
    fn keeps_results_and_errors() {
        let mut findings = Findings::default();
        assert!(findings.record("launch", Ok(Probe(42.0))).is_some());
        assert!(
            findings
                .record::<Probe>("idle", Err(anyhow::anyhow!("Android is not running")))
                .is_none()
        );
        assert_eq!(findings.metrics["launch.splash"].value, 42.0);
        assert_eq!(findings.steps["launch"], serde_json::json!(42.0));
        assert_eq!(findings.failures["idle"], "Android is not running");
    }
}
