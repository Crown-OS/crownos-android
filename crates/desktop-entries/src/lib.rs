//! Mirrors the desktop entries Waydroid's user manager writes into the
//! session's private data home into the user's menu, rewritten to launch
//! through `crown-android`, installs the handlers for APK files and
//! `market:` links, and hides the waydroid package's own entries.
//!
//! The rewrites are pure functions over the entry text; [`mirror::sync`] is
//! the only part that touches the disk.

mod entry;
mod handler;
pub mod mirror;
mod overrides;
mod package;
mod rewrite;
mod util;

pub use entry::DesktopEntry;
pub use handler::{HANDLERS, Handler};
pub use mirror::{AppInfo, SyncReport, app_info, install_handlers, remove_all, sync};
pub use overrides::{
    WAYDROID_ENTRIES, hidden_override, hide_waydroid_entries, unhide_waydroid_entries,
};
pub use package::{file_of_package, is_package_name, package_of_file};
pub use rewrite::mirror_entry;

/// Group of the main entry.
pub const MAIN_GROUP: &str = "Desktop Entry";
/// Key every mirror carries, naming its package. Only entries with it are
/// ever deleted.
pub const MIRROR_MARKER: &str = "X-CrownOS-Android";
