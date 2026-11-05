//! Methods `crown-androidd` serves on the `crownos-android` socket next to
//! contract 4 ([`android_protocol::android`]), for `crown-android install`,
//! `open` and `settings`. Selectors hash the service, name and signature, so
//! they never collide with the contract's; the daemon routes them first and
//! hands everything else to `android::dispatch`.

use android_protocol::LaunchOutcome;

crownos_ipc::protocol! {
    /// Requests beyond contract 4.
    pub mod supplement: "crownos-android" {
        /// Install `file_name`, which the caller already copied into the
        /// inbox (`<android data>/crownos/inbox/`). The result is a notification.
        fn install_package(file_name: String) -> LaunchOutcome;
        /// Open a `market:` or `https:` link in Android.
        fn open_uri(uri: String) -> LaunchOutcome;
        /// Show the Android settings page of `package`.
        fn open_app_settings(package: String) -> LaunchOutcome;
    }
}
