//! The user settings the daemon reads: the idle timeout, followed live, and
//! the desktop's light or dark tone for the splash.

use crownos_config::Subscription;
use crownos_config::schema::{Android, Appearance, android};
use splash::Tone;

use crate::notice::{Notice, Notifier};

pub fn idle_minutes() -> u32 {
    crownos_config::load::<Android>(Android::SECTION).idle_minutes
}

/// Reports every later change of `idle_minutes`.
#[must_use = "dropping the subscription stops following android.ron"]
pub fn follow_idle_minutes(notifier: Notifier) -> Subscription {
    crownos_config::subscribe_key(android::IdleMinutes, move |minutes| {
        notifier.notify(Notice::IdleMinutes(minutes));
    })
}

pub fn tone() -> Tone {
    if crownos_config::load::<Appearance>(Appearance::SECTION).dark_mode {
        Tone::Dark
    } else {
        Tone::Light
    }
}
