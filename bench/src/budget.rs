use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::findings::MetricId;
use crate::unit::Unit;

/// The budgets file shipped with the bench, used unless `--budgets` names another.
const DEFAULT_BUDGETS: &str = include_str!("../budgets/budgets.toml");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bound {
    /// The number must stay at or below the limit.
    #[default]
    Max,
    /// The number must stay at or above the limit.
    Min,
}

impl Bound {
    /// How much worse `value` is than `reference`; negative when it is better.
    pub fn worse_by(self, value: f64, reference: f64) -> f64 {
        match self {
            Self::Max => value - reference,
            Self::Min => reference - value,
        }
    }
}

/// One `[area.name]` table of budgets.toml.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub unit: Unit,
    pub limit: f64,
    #[serde(default)]
    pub bound: Bound,
    /// Added to `limit` for every Android app with a window beyond the first.
    #[serde(default)]
    pub per_extra_app: f64,
    /// A change from the baseline this small is noise, not a regression.
    #[serde(default)]
    pub noise: f64,
    /// How the number is taken.
    pub method: String,
}

impl Budget {
    /// The limit while `android_apps` apps have a window open.
    pub fn limit_for(&self, android_apps: Option<u32>) -> f64 {
        let extra_apps = android_apps.map_or(0, |apps| apps.saturating_sub(1));
        self.limit + self.per_extra_app * f64::from(extra_apps)
    }
}

/// Budgets by id: `<area>.<name>`, from the `[area.name]` tables.
#[derive(Debug)]
pub struct Budgets(BTreeMap<String, Budget>);

impl Budgets {
    pub fn parse(text: &str) -> Result<Self> {
        let areas: BTreeMap<String, BTreeMap<String, Budget>> = toml::from_str(text)?;
        Ok(Self(
            areas
                .into_iter()
                .flat_map(|(area, budgets)| {
                    budgets
                        .into_iter()
                        .map(move |(name, budget)| (format!("{area}.{name}"), budget))
                })
                .collect(),
        ))
    }

    /// `path`, or the budgets built into the bench.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let Some(path) = path else {
            return Self::parse(DEFAULT_BUDGETS).context("the built-in budgets are invalid");
        };
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        Self::parse(&text)
            .with_context(|| format!("{} is not a valid budgets file", path.display()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Budget)> {
        self.0.iter().map(|(id, budget)| (id.as_str(), budget))
    }

    /// Fails unless every number the bench takes has a budget of the same kind,
    /// so a run never ends with numbers nothing judges.
    pub fn check_covers(&self, metrics: &[MetricId]) -> Result<()> {
        for metric in metrics {
            let Some(budget) = self.0.get(metric.name) else {
                bail!("the budgets have no entry for {}", metric.name);
            };
            if !budget.unit.same_kind(metric.unit) {
                bail!(
                    "budget {} is in {}, but the bench measures it in {}",
                    metric.name,
                    budget.unit.symbol(),
                    metric.unit.symbol()
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::METRICS;

    const PLAN_AREAS: [&str; 7] = [
        "launch",
        "resize",
        "memory",
        "idle",
        "idle_stop",
        "frames",
        "setup",
    ];

    #[test]
    fn the_shipped_budgets_cover_the_plan_and_every_metric() -> Result<()> {
        let budgets = Budgets::load(None)?;
        budgets.check_covers(&METRICS)?;
        for area in PLAN_AREAS {
            assert!(
                budgets
                    .iter()
                    .any(|(id, _)| id.starts_with(&format!("{area}."))),
                "no budget in {area}"
            );
        }
        let Some((_, scroll)) = budgets.iter().find(|(id, _)| *id == "frames.scroll") else {
            panic!("frames.scroll is missing");
        };
        assert_eq!(scroll.bound, Bound::Min);
        Ok(())
    }

    #[test]
    fn rejects_unknown_fields_and_missing_units() {
        let typo = "[launch.splash]\nunit = \"ms\"\nlimt = 100\nmethod = \"m\"\n";
        assert!(Budgets::parse(typo).is_err());
        let no_unit = "[launch.splash]\nlimit = 100\nmethod = \"m\"\n";
        assert!(Budgets::parse(no_unit).is_err());
    }

    #[test]
    fn rejects_budgets_of_another_kind() -> Result<()> {
        let budgets =
            Budgets::parse("[idle.android_cpu]\nunit = \"ms\"\nlimit = 2\nmethod = \"m\"\n")?;
        let cpu = MetricId {
            name: "idle.android_cpu",
            unit: Unit::Percent,
        };
        assert!(budgets.check_covers(&[cpu]).is_err());
        Ok(())
    }

    #[test]
    fn grows_the_limit_per_extra_app() -> Result<()> {
        let budgets = Budgets::parse(
            "[memory.android]\nunit = \"MiB\"\nlimit = 1200\nper_extra_app = 250\nmethod = \"m\"\n",
        )?;
        let Some((_, memory)) = budgets.iter().next() else {
            panic!("no budget parsed");
        };
        assert_eq!(memory.limit_for(None), 1200.0);
        assert_eq!(memory.limit_for(Some(0)), 1200.0);
        assert_eq!(memory.limit_for(Some(1)), 1200.0);
        assert_eq!(memory.limit_for(Some(3)), 1700.0);
        Ok(())
    }
}
