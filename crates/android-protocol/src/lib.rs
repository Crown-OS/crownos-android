//! Control protocol of `crown-androidd` over `crownos-ipc`
//! (`$XDG_RUNTIME_DIR/crownos/crownos-android.sock`).
//!
//! Clients are `crown-android launch` (the desktop entries' `Exec`), the Settings
//! page and the shell. The daemon starts Android on demand, so a client never
//! needs to know whether Android is running before it calls [`android::Client`].
//!
//! New fields only ever go last: postcard ignores trailing bytes, so older
//! clients still decode newer records.

mod runtime;

pub use runtime::{GpuTier, LaunchOutcome, RuntimeState, RuntimeStatus, StopReason};
pub use setup_state::SetupStatus;

/// Prefix of every Android window's `app_id` and desktop file id:
/// `waydroid.<package>`.
pub const APP_ID_PREFIX: &str = "waydroid.";

/// The `app_id` and desktop file id of `package`.
pub fn app_id(package: &str) -> String {
    format!("{APP_ID_PREFIX}{package}")
}

/// The Android package behind an `app_id`, or `None` for a non-Android window.
pub fn package_of(app_id: &str) -> Option<&str> {
    app_id
        .strip_prefix(APP_ID_PREFIX)
        .filter(|package| !package.is_empty())
}

crownos_ipc::protocol! {
    /// `crown-androidd`.
    pub mod android: "crownos-android" {
        /// Open `package` in a new window, starting Android first when it is
        /// stopped. `activation_token` is the launcher's xdg-activation token.
        fn launch(package: String, activation_token: Option<String>) -> LaunchOutcome;
        /// Stop Android now instead of waiting for the idle timeout.
        fn stop(reason: StopReason);
        fn status() -> RuntimeStatus;
        /// Write a diagnostics archive and return its path.
        fn diagnose() -> String;
        /// The Google services framework id to register at
        /// <https://www.google.com/android/uncertified>, once Android has one.
        fn google_registration_id() -> Option<String>;
        /// Install `file_name`, which the caller already copied into the
        /// inbox (`<android data>/crownos/inbox/`). The result is a notification.
        fn install_package(file_name: String) -> LaunchOutcome;
        /// Open a `market:` or `https:` link in Android.
        fn open_uri(uri: String) -> LaunchOutcome;
        /// Show the Android settings page of `package`.
        fn open_app_settings(package: String) -> LaunchOutcome;
        event RuntimeChanged { status: RuntimeStatus };
        event SetupProgress { status: SetupStatus };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids_round_trip_to_packages() {
        assert_eq!(app_id("com.whatsapp"), "waydroid.com.whatsapp");
        assert_eq!(package_of("waydroid.com.whatsapp"), Some("com.whatsapp"));
    }

    #[test]
    fn other_windows_have_no_package() {
        assert_eq!(package_of("org.crownos.files"), None);
        assert_eq!(package_of("waydroid."), None);
    }
}
