//! Waydroid's own files: `waydroid.cfg`, the LXC `config_nodes`, the OTA
//! channel, and the `waydroid init` step that ties them together.

pub mod cfg;
pub mod channel;
pub mod config_nodes;
pub mod init;

pub use cfg::WaydroidConfig;

/// `waydroid.cfg` section with Waydroid's own settings.
pub const WAYDROID_SECTION: &str = "waydroid";
/// `waydroid.cfg` section whose keys become Android properties.
pub const PROPERTIES_SECTION: &str = "properties";
