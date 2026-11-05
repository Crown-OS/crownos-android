//! `crownos-android-setup enable`, run by Settings through pkexec after a
//! reset or to turn Android apps back on.

use std::fs;

use crownos_config::system::AndroidConfig;
use setup_state::{SetupStatus, Stage};

use super::SETUP_UNIT;
use crate::{Host, Paths, android_ron};

/// Turns Android apps on, queues setup and starts it without waiting.
pub fn enable(paths: &Paths, host: &dyn Host) -> anyhow::Result<()> {
    let config = enabled(android_ron::load(&paths.android_ron())?);
    android_ron::save(&paths.android_ron(), &config)?;
    let marker = paths.pending_marker();
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&marker, "")?;
    setup_state::store(
        &paths.status(),
        &SetupStatus::new(Stage::Queued, None, "Waiting to set up Android"),
    )?;
    if let Err(error) = host.systemctl(&["enable", SETUP_UNIT]) {
        log::warn!("could not enable {SETUP_UNIT}: {error:#}");
    }
    host.systemctl(&["start", "--no-block", SETUP_UNIT])
}

/// `config` with Android apps on and every other choice kept. Without a file
/// the result is what the installer writes for "without Google Play".
pub fn enabled(config: Option<AndroidConfig>) -> AndroidConfig {
    AndroidConfig {
        enabled: true,
        ..config.unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crownos_config::system::AndroidTier;

    use super::*;

    #[test]
    fn enabling_keeps_every_other_choice() {
        let removed = AndroidConfig {
            enabled: false,
            google_play: true,
            tier: Some(AndroidTier::Venus),
            bundle: Some("2026.11.0".into()),
        };
        let again = enabled(Some(removed.clone()));
        assert_eq!(
            again,
            AndroidConfig {
                enabled: true,
                ..removed
            }
        );
        assert_eq!(enabled(Some(again.clone())), again);
    }

    #[test]
    fn a_missing_file_becomes_an_enabled_default() {
        let created = enabled(None);
        assert!(created.enabled && !created.google_play);
        assert_eq!((created.tier, created.bundle), (None, None));
    }
}
