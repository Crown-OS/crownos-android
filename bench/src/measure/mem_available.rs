use anyhow::{Context, Result};
use serde::Serialize;

use crate::findings::{Findings, MetricId, Report};
use crate::host::Host;
use crate::unit::Unit;
use crate::util::number::mib_from_kib;

pub const LOST: MetricId = MetricId {
    name: "idle_stop.mem_available",
    unit: Unit::Mebibytes,
};

/// MemAvailable before Android started and after it stopped.
#[derive(Debug, Serialize)]
pub struct MemAvailableDelta {
    pub reference_kib: u64,
    pub after_stop_kib: u64,
    /// Positive when less is available than before Android started.
    pub lost_mib: f64,
}

/// MemAvailable now, while Android is stopped.
pub fn reference(host: &Host) -> Result<u64> {
    host.mem_available_kib()
        .context("MemAvailable is unreadable")
}

pub fn after_stop(host: &Host, reference_kib: u64) -> Result<MemAvailableDelta> {
    let after_stop_kib = reference(host)?;
    Ok(MemAvailableDelta {
        reference_kib,
        after_stop_kib,
        lost_mib: mib_from_kib(reference_kib as f64 - after_stop_kib as f64),
    })
}

impl Report for MemAvailableDelta {
    fn report(&self, _step: &str, findings: &mut Findings) {
        findings.metric(LOST, self.lost_mib);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::FakeHost;

    #[test]
    fn reports_what_the_cycle_lost() -> anyhow::Result<()> {
        let fake = FakeHost::new("mem-available")?;
        fake.file(
            "proc/meminfo",
            "MemTotal: 16000000 kB\nMemAvailable: 8000000 kB\n",
        )?;
        let delta = after_stop(&fake.host(), 8_030_720)?;
        assert_eq!(delta.lost_mib, 30.0);
        let mut findings = Findings::default();
        delta.report("mem_available", &mut findings);
        assert_eq!(findings.metrics[LOST.name].value, 30.0);
        assert_eq!(after_stop(&fake.host(), 7_948_800)?.lost_mib, -50.0);
        Ok(())
    }
}
