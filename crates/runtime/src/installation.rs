//! Whether this machine has Android apps set up, from `/etc/crownos/android.ron`,
//! the pending marker and `setup.json`.

use std::path::Path;

use android_protocol::GpuTier;
use crownos_config::system::{AndroidConfig, AndroidTier};
use setup_state::{FailureCode, SetupStatus, Stage};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Installation {
    /// Android apps were not chosen, or were removed.
    NotInstalled,
    /// The setup service still has work to do.
    SettingUp,
    /// Setup found that this machine cannot run Android.
    Unsupported {
        detail: String,
    },
    Installed(Installed),
}

/// What setup left behind for the runtime.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Installed {
    pub tier: Option<GpuTier>,
    /// The bundle version crown-agent must report in `Hello`.
    pub bundle: Option<String>,
    pub google_play: bool,
}

impl Installed {
    pub fn venus(&self) -> bool {
        self.tier == Some(GpuTier::Venus)
    }
}

impl Installation {
    /// Combines the three sources. `pending` is whether the marker exists.
    pub fn derive(
        config: Option<&AndroidConfig>,
        pending: bool,
        setup: Option<&SetupStatus>,
    ) -> Self {
        let Some(config) = config.filter(|config| config.enabled) else {
            return Self::NotInstalled;
        };
        if let Some(detail) = setup.and_then(unsupported_detail) {
            return Self::Unsupported { detail };
        }
        if pending {
            return Self::SettingUp;
        }
        Self::Installed(Installed {
            tier: config.tier.map(gpu_tier),
            bundle: config.bundle.clone(),
            google_play: config.google_play,
        })
    }

    /// Reads the system files at their contract paths below `root` (`/` on a
    /// real system).
    pub fn load(root: &Path) -> Self {
        let config_path =
            root.join(crownos_config::system::ANDROID_CONFIG_PATH.trim_start_matches('/'));
        let config = AndroidConfig::load_from(&config_path).unwrap_or_else(|error| {
            log::warn!("{error}");
            None
        });
        let pending = root
            .join(setup_state::PENDING_MARKER.trim_start_matches('/'))
            .exists();
        let setup = load_setup(root);
        Self::derive(config.as_ref(), pending, setup.as_ref())
    }

    pub fn installed(&self) -> Option<&Installed> {
        match self {
            Self::Installed(installed) => Some(installed),
            _ => None,
        }
    }
}

/// The current `setup.json` below `root`, if any can be read.
pub fn load_setup(root: &Path) -> Option<SetupStatus> {
    let path = root.join(setup_state::STATUS_PATH.trim_start_matches('/'));
    setup_state::load(&path).unwrap_or_else(|error| {
        log::warn!("{error}");
        None
    })
}

fn unsupported_detail(setup: &SetupStatus) -> Option<String> {
    let failure = setup
        .failure
        .as_ref()
        .filter(|_| setup.stage == Stage::Failed)?;
    matches!(
        failure.code,
        FailureCode::NoBinder | FailureCode::UnsupportedPageSize
    )
    .then(|| failure.detail.clone())
}

const fn gpu_tier(tier: AndroidTier) -> GpuTier {
    match tier {
        AndroidTier::Native => GpuTier::Native,
        AndroidTier::Venus => GpuTier::Venus,
        AndroidTier::Software => GpuTier::Software,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use setup_state::Failure;

    fn config() -> AndroidConfig {
        AndroidConfig {
            enabled: true,
            google_play: true,
            tier: Some(AndroidTier::Venus),
            bundle: Some("2026.11.0".into()),
        }
    }

    #[test]
    fn no_file_or_a_disabled_one_means_not_installed() {
        assert_eq!(
            Installation::derive(None, true, None),
            Installation::NotInstalled
        );
        let disabled = AndroidConfig {
            enabled: false,
            ..config()
        };
        assert_eq!(
            Installation::derive(Some(&disabled), false, None),
            Installation::NotInstalled
        );
    }

    #[test]
    fn the_pending_marker_means_setting_up() {
        let downloading = SetupStatus::new(Stage::Downloading, Some(3), "Downloading");
        assert_eq!(
            Installation::derive(Some(&config()), true, Some(&downloading)),
            Installation::SettingUp
        );
    }

    #[test]
    fn a_machine_failure_means_unsupported() {
        let failed = SetupStatus::failed(Failure {
            code: FailureCode::UnsupportedPageSize,
            detail: "This kernel uses 16 KB pages.".into(),
            retryable: false,
        });
        assert_eq!(
            Installation::derive(Some(&config()), true, Some(&failed)),
            Installation::Unsupported {
                detail: "This kernel uses 16 KB pages.".into()
            }
        );
    }

    #[test]
    fn a_finished_setup_carries_tier_and_bundle() {
        let ready = SetupStatus::new(Stage::Ready, Some(100), "Ready");
        let installation = Installation::derive(Some(&config()), false, Some(&ready));
        let installed = installation.installed().cloned().unwrap_or_default();
        assert!(installed.venus());
        assert_eq!(installed.bundle.as_deref(), Some("2026.11.0"));
        assert!(installed.google_play);
    }
}
