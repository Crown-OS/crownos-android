//! Turning the daemon's answers into output and an exit status.

use android_protocol::{LaunchOutcome, RuntimeStatus};
use anyhow::{Result, bail};

/// Success for `Opening` and `Starting`, an error naming the reason otherwise.
pub fn check(outcome: LaunchOutcome) -> Result<()> {
    match outcome {
        LaunchOutcome::Opening | LaunchOutcome::Starting => Ok(()),
        LaunchOutcome::NotInstalled => bail!("Android apps are not installed"),
        LaunchOutcome::SettingUp => bail!("Android apps are still being set up"),
        LaunchOutcome::Unsupported => bail!("this machine cannot run Android apps"),
        LaunchOutcome::UnknownPackage => bail!("Android does not know this app"),
    }
}

pub fn describe(status: &RuntimeStatus) -> String {
    let mut text = format!("state: {:?}\nwindows: {}\n", status.state, status.windows);
    if let Some(at) = status.idle_stop_at_ms {
        text.push_str(&format!("idle stop at: {at} (Unix ms)\n"));
    }
    if let Some(tier) = status.tier {
        text.push_str(&format!("gpu tier: {tier:?}\n"));
    }
    if let Some(bundle) = &status.bundle_version {
        text.push_str(&format!("bundle: {bundle}\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_opening_and_starting_succeed() {
        assert!(check(LaunchOutcome::Opening).is_ok());
        assert!(check(LaunchOutcome::Starting).is_ok());
        for failed in [
            LaunchOutcome::NotInstalled,
            LaunchOutcome::SettingUp,
            LaunchOutcome::Unsupported,
            LaunchOutcome::UnknownPackage,
        ] {
            assert!(check(failed).is_err());
        }
    }

    #[test]
    fn describes_a_stopped_runtime() {
        assert_eq!(
            describe(&RuntimeStatus::stopped()),
            "state: Stopped\nwindows: 0\n"
        );
    }
}
