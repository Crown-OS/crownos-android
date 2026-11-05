//! What each notification says: pure functions of the setup state.

use setup_state::{SetupStatus, Stage};

/// Opens `https://www.google.com/android/uncertified` and copies the id.
pub const REGISTER: &str = "register";
pub const OPEN_PLAY_STORE: &str = "open-play-store";
pub const OPEN_SETTINGS: &str = "open-settings";
/// Starts the setup service again.
pub const RETRY_SETUP: &str = "retry-setup";
/// Runs the first boot again.
pub const RETRY_WARM_UP: &str = "retry-warm-up";

/// A notification card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub summary: String,
    pub body: String,
    /// `None` spins; `Some(100)` with actions ends a progress card.
    pub percent: Option<u8>,
    /// Action id and label pairs.
    pub actions: Vec<(&'static str, &'static str)>,
}

impl Content {
    fn new(summary: &str, body: &str, percent: Option<u8>) -> Self {
        Self {
            summary: summary.to_owned(),
            body: body.to_owned(),
            percent,
            actions: Vec::new(),
        }
    }

    fn with(mut self, action: &'static str, label: &'static str) -> Self {
        self.actions.push((action, label));
        self
    }

    /// The flat id, label list freedesktop and crownotify take.
    pub fn action_list(&self) -> Vec<String> {
        self.actions
            .iter()
            .flat_map(|(id, label)| [(*id).to_owned(), (*label).to_owned()])
            .collect()
    }
}

/// Where Android setup is, from the user's point of view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupNotice {
    /// The root service reports progress.
    Progress(SetupStatus),
    /// The first boot runs.
    Finishing,
    Ready {
        google_play: bool,
    },
    WarmUpFailed {
        reason: String,
    },
}

/// The card for a setup state.
pub fn setup_card(notice: &SetupNotice) -> Content {
    match notice {
        SetupNotice::Progress(status) => match (&status.stage, &status.failure) {
            (Stage::Failed, Some(failure)) => {
                let card = Content::new(
                    "Android apps could not be set up",
                    &failure.detail,
                    Some(100),
                );
                if failure.retryable {
                    card.with(RETRY_SETUP, "Try again")
                } else {
                    card
                }
            }
            (Stage::Ready, _) => finishing(),
            _ => Content::new("Setting up Android apps", &status.message, status.percent),
        },
        SetupNotice::Finishing => finishing(),
        SetupNotice::Ready { google_play: true } => Content::new(
            "Android apps are ready",
            "Register this device with Google to use the Play Store.",
            Some(100),
        )
        .with(REGISTER, "Register this device")
        .with(OPEN_PLAY_STORE, "Open Play Store"),
        SetupNotice::Ready { google_play: false } => Content::new(
            "Android apps are ready",
            "Install apps from an APK file or a store app.",
            Some(100),
        )
        .with(OPEN_SETTINGS, "Open Settings"),
        SetupNotice::WarmUpFailed { reason } => {
            Content::new("Android apps could not start", reason, Some(100))
                .with(RETRY_WARM_UP, "Try again")
        }
    }
}

fn finishing() -> Content {
    Content::new(
        "Finishing setup",
        "Starting Android for the first time",
        None,
    )
}

/// One-off cards that are not part of setup.
pub fn safe_mode_card(reason: &str) -> Content {
    Content::new(
        "Android started in safe mode",
        &format!("{reason} Android apps open in plain windows until this is fixed."),
        None,
    )
}

pub fn install_card(ok: bool, message: &str) -> Content {
    if ok {
        Content::new("App installed", message, None)
    } else {
        Content::new("The app could not be installed", message, None)
    }
}

pub fn registration_card(copied: bool) -> Content {
    if copied {
        Content::new(
            "Device id copied",
            "Paste it into the page that opened, then restart the Play Store.",
            None,
        )
    } else {
        Content::new(
            "No device id yet",
            "Open the Play Store once, then try registering again.",
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use setup_state::{Failure, FailureCode};

    #[test]
    fn progress_spins_until_setup_knows_a_percentage() {
        let waiting = SetupStatus::new(Stage::WaitingForNetwork, None, "Waiting for the network");
        let card = setup_card(&SetupNotice::Progress(waiting));
        assert_eq!(card.percent, None);
        assert_eq!(card.body, "Waiting for the network");
        let downloading = SetupStatus::new(Stage::Downloading, Some(42), "Downloading Android");
        assert_eq!(
            setup_card(&SetupNotice::Progress(downloading)).percent,
            Some(42)
        );
    }

    #[test]
    fn a_retryable_failure_offers_to_try_again() {
        let failed = |retryable| {
            SetupStatus::failed(Failure {
                code: FailureCode::Network,
                detail: "The download stopped.".into(),
                retryable,
            })
        };
        let card = setup_card(&SetupNotice::Progress(failed(true)));
        assert_eq!(card.body, "The download stopped.");
        assert_eq!(card.action_list(), ["retry-setup", "Try again"]);
        assert!(
            setup_card(&SetupNotice::Progress(failed(false)))
                .actions
                .is_empty()
        );
    }

    #[test]
    fn the_root_part_finishing_shows_the_warm_up() {
        let ready = SetupStatus::new(Stage::Ready, Some(100), "Ready");
        assert_eq!(
            setup_card(&SetupNotice::Progress(ready)),
            setup_card(&SetupNotice::Finishing)
        );
        assert_eq!(
            setup_card(&SetupNotice::Finishing).summary,
            "Finishing setup"
        );
    }

    #[test]
    fn google_play_asks_for_registration() {
        let card = setup_card(&SetupNotice::Ready { google_play: true });
        assert_eq!(card.summary, "Android apps are ready");
        assert_eq!(card.percent, Some(100));
        assert_eq!(
            card.action_list(),
            [
                "register",
                "Register this device",
                "open-play-store",
                "Open Play Store"
            ]
        );
        let card = setup_card(&SetupNotice::Ready { google_play: false });
        assert_eq!(card.action_list(), ["open-settings", "Open Settings"]);
    }
}
