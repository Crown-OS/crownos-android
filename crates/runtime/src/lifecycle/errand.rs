/// Package that opens `market:` links.
const PLAY_STORE: &str = "com.android.vending";
/// Package whose window shows an app's settings page.
pub const SETTINGS: &str = "com.android.settings";

/// Work that needs Android running. Queued while it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Errand {
    /// Open `package` in a window, or raise its window.
    Launch {
        package: String,
        token: Option<String>,
    },
    /// Open a `market:` or `https:` link in Android.
    OpenUri { uri: String },
    /// Install the APK at this `/data/crownos/inbox/` path.
    Install { guest_path: String },
    /// Show the settings page of `package`.
    AppSettings { package: String },
}

impl Errand {
    /// The app whose splash covers the wait, if one opens a window.
    pub fn splash_package(&self) -> Option<&str> {
        match self {
            Self::Launch { package, .. } => Some(package),
            Self::AppSettings { .. } => Some(SETTINGS),
            Self::OpenUri { uri } if uri.starts_with("market:") => Some(PLAY_STORE),
            Self::OpenUri { .. } | Self::Install { .. } => None,
        }
    }
}
