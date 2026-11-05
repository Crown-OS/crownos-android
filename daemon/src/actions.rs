//! What the buttons on the daemon's notifications do.

use std::time::Instant;

use runtime::Errand;
use runtime::lifecycle::SETTINGS;

use crate::daemon::Daemon;
use crate::host_worker::Job;
use crate::notify::UserAction;
use crate::notify::content::{
    OPEN_PLAY_STORE, OPEN_SETTINGS, REGISTER, RETRY_SETUP, RETRY_WARM_UP, registration_card,
};
use crate::util::open_url::open_url;

const PLAY_STORE: &str = "com.android.vending";
/// Google's form for registering an uncertified device by its services id.
const UNCERTIFIED_DEVICE_PAGE: &str = "https://www.google.com/android/uncertified";

impl Daemon {
    pub fn handle_user_action(&mut self, action: UserAction, now: Instant) {
        let (id, action) = match action {
            UserAction::Closed { id } => {
                self.notifications.forget(id);
                return;
            }
            UserAction::Invoked { id, action } => (id, action),
        };
        if self.notifications.setup_id() != Some(id) {
            return;
        }
        match action.as_str() {
            RETRY_SETUP => self.host.submit(Job::RetrySetup),
            RETRY_WARM_UP => self.lifecycle.resume_warm_up(now),
            REGISTER => self.register_device(now),
            OPEN_PLAY_STORE => self.open_app(PLAY_STORE, now),
            OPEN_SETTINGS => self.open_app(SETTINGS, now),
            other => log::debug!("ignoring notification action {other}"),
        }
    }

    fn open_app(&mut self, package: &str, now: Instant) {
        let errand = Errand::Launch {
            package: package.to_owned(),
            token: None,
        };
        let outcome = self.lifecycle.submit(errand, now);
        log::info!("opening {package}: {outcome:?}");
    }

    /// Copies the services id and opens Google's registration page.
    fn register_device(&mut self, now: Instant) {
        let id = self.lifecycle.persisted().gsf_id.clone();
        let copied = match (id, self.wayland.get(now)) {
            (Some(id), Some(splashes)) => splashes
                .copy_text(id)
                .inspect_err(|error| log::warn!("could not copy the device id: {error}"))
                .is_ok(),
            _ => false,
        };
        open_url(UNCERTIFIED_DEVICE_PAGE);
        self.notifications.post(&registration_card(copied));
    }
}
