use std::fmt;

use serde::{Deserialize, Serialize};

/// The unit of a budget limit or a measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[serde(rename = "ms")]
    Milliseconds,
    #[serde(rename = "s")]
    Seconds,
    #[serde(rename = "MiB")]
    Mebibytes,
    /// Percent of one CPU core.
    #[serde(rename = "percent")]
    Percent,
    #[serde(rename = "per_second")]
    PerSecond,
    #[serde(rename = "count")]
    Count,
    #[serde(rename = "fps")]
    FramesPerSecond,
}

const MILLISECONDS_PER_SECOND: f64 = 1000.0;

impl Unit {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Milliseconds => "ms",
            Self::Seconds => "s",
            Self::Mebibytes => "MiB",
            Self::Percent => "%",
            Self::PerSecond => "/s",
            Self::Count => "",
            Self::FramesPerSecond => "fps",
        }
    }

    /// The finest unit of the same kind, and how many of it make one `self`.
    const fn base(self) -> (Self, f64) {
        match self {
            Self::Seconds => (Self::Milliseconds, MILLISECONDS_PER_SECOND),
            unit => (unit, 1.0),
        }
    }

    pub fn same_kind(self, other: Self) -> bool {
        self.base().0 == other.base().0
    }

    /// `value` in `self` expressed in `target`, or `None` across kinds.
    pub fn convert(self, value: f64, target: Self) -> Option<f64> {
        let (base, factor) = self.base();
        let (target_base, target_factor) = target.base();
        (base == target_base).then(|| value * factor / target_factor)
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_within_a_kind_only() {
        assert_eq!(Unit::Milliseconds.convert(1500.0, Unit::Seconds), Some(1.5));
        assert_eq!(
            Unit::Seconds.convert(12.0, Unit::Milliseconds),
            Some(12_000.0)
        );
        assert_eq!(Unit::Mebibytes.convert(3.0, Unit::Mebibytes), Some(3.0));
        assert_eq!(Unit::Mebibytes.convert(3.0, Unit::Milliseconds), None);
        assert!(Unit::Seconds.same_kind(Unit::Milliseconds));
        assert!(!Unit::Percent.same_kind(Unit::PerSecond));
    }
}
