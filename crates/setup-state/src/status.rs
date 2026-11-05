use serde::{Deserialize, Serialize};

/// Format version of [`SetupStatus`]. Bumped only for changes an older reader
/// cannot ignore.
pub const FORMAT: u32 = 1;

/// Setup stages in the order the service runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Queued,
    WaitingForNetwork,
    Probing,
    Downloading,
    Verifying,
    Initializing,
    InstallingBundle,
    /// The root part is done. `crown-androidd` runs the first boot next.
    Ready,
    Failed,
}

impl Stage {
    /// Whether the service has stopped working on its own.
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    NoBinder,
    UnsupportedPageSize,
    NotEnoughDisk,
    Network,
    Checksum,
    Signature,
    Init,
    Bundle,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub code: FailureCode,
    /// One sentence for the notification body.
    pub detail: String,
    /// Whether starting the service again can succeed without changing the machine.
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupStatus {
    pub format: u32,
    pub stage: Stage,
    /// Progress over the whole setup. `None` means the loader spins.
    pub percent: Option<u8>,
    /// One short line for the notification.
    pub message: String,
    pub failure: Option<Failure>,
}

impl SetupStatus {
    pub fn new(stage: Stage, percent: Option<u8>, message: impl Into<String>) -> Self {
        Self {
            format: FORMAT,
            stage,
            percent: percent.map(|value| value.min(100)),
            message: message.into(),
            failure: None,
        }
    }

    pub fn failed(failure: Failure) -> Self {
        Self {
            format: FORMAT,
            stage: Stage::Failed,
            percent: None,
            message: failure.detail.clone(),
            failure: Some(failure),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = r#"{
  "format": 1,
  "stage": "downloading",
  "percent": 42,
  "message": "Downloading Android",
  "failure": null
}"#;

    #[test]
    fn matches_the_golden_record() -> Result<(), serde_json::Error> {
        let status = SetupStatus::new(Stage::Downloading, Some(42), "Downloading Android");
        assert_eq!(serde_json::to_string_pretty(&status)?, GOLDEN);
        assert_eq!(serde_json::from_str::<SetupStatus>(GOLDEN)?, status);
        Ok(())
    }

    #[test]
    fn percent_never_exceeds_one_hundred() {
        assert_eq!(
            SetupStatus::new(Stage::Verifying, Some(250), "").percent,
            Some(100)
        );
    }

    #[test]
    fn a_failure_ends_setup() {
        let status = SetupStatus::failed(Failure {
            code: FailureCode::Network,
            detail: "The download stopped.".into(),
            retryable: true,
        });
        assert!(status.stage.is_final());
        assert_eq!(status.message, "The download stopped.");
    }
}
