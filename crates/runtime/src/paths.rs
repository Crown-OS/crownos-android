//! Where one user's Android runtime keeps its files on the host (contract 10).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Directory of the system setup service: `pending`, `setup.json`, images.
pub const SETUP_DIR: &str = "/var/lib/crownos/android";

/// Name of the Waydroid directory inside an `XDG_DATA_HOME`.
const WAYDROID: &str = "waydroid";
const LOCAL_SHARE: &str = ".local/share";

#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    #[error("{0} is not set to an absolute path")]
    Unset(&'static str),
}

/// The user directories every path derives from. The contract paths hang
/// off `home` exactly as the session unit names them (`%h/.local/share/...`);
/// only the menu follows `$XDG_DATA_HOME`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub home: PathBuf,
    /// `$XDG_DATA_HOME`, usually `~/.local/share`: where menu entries go.
    pub data_dir: PathBuf,
    /// `$XDG_RUNTIME_DIR`.
    pub runtime_dir: PathBuf,
}

impl Paths {
    /// The standard layout below `home`.
    pub fn under_home(home: &Path, runtime_dir: &Path) -> Self {
        Self {
            home: home.to_owned(),
            data_dir: home.join(LOCAL_SHARE),
            runtime_dir: runtime_dir.to_owned(),
        }
    }

    /// Reads `HOME`, `XDG_RUNTIME_DIR` and `XDG_DATA_HOME`.
    pub fn from_env() -> Result<Self, PathsError> {
        let home = absolute(std::env::var_os("HOME")).ok_or(PathsError::Unset("HOME"))?;
        let runtime_dir = absolute(std::env::var_os("XDG_RUNTIME_DIR"))
            .ok_or(PathsError::Unset("XDG_RUNTIME_DIR"))?;
        let data_dir =
            absolute(std::env::var_os("XDG_DATA_HOME")).unwrap_or_else(|| home.join(LOCAL_SHARE));
        Ok(Self {
            home,
            data_dir,
            runtime_dir,
        })
    }

    /// The private `XDG_DATA_HOME` of the Waydroid session.
    pub fn session_data_home(&self) -> PathBuf {
        self.home.join(LOCAL_SHARE).join("crownos/android")
    }

    /// `waydroid` inside the session data home, a symlink to [`Self::waydroid_state`].
    pub fn session_waydroid_link(&self) -> PathBuf {
        self.session_data_home().join(WAYDROID)
    }

    /// The user's real Waydroid directory, `~/.local/share/waydroid`.
    pub fn waydroid_state(&self) -> PathBuf {
        self.home.join(LOCAL_SHARE).join(WAYDROID)
    }

    /// Android's `/data` as the host sees it.
    pub fn android_data(&self) -> PathBuf {
        self.waydroid_state().join("data")
    }

    /// `/data/crownos/agent.sock` as the host sees it.
    pub fn agent_socket(&self) -> PathBuf {
        self.android_data().join("crownos/agent.sock")
    }

    /// `/data/crownos/inbox/` as the host sees it.
    pub fn inbox(&self) -> PathBuf {
        self.android_data().join("crownos/inbox")
    }

    /// The icons Waydroid exports, one `<package>.png` each.
    pub fn icons(&self) -> PathBuf {
        self.android_data().join("icons")
    }

    /// Where Waydroid's user manager writes `waydroid.<package>.desktop`.
    pub fn waydroid_applications(&self) -> PathBuf {
        self.session_data_home().join("applications")
    }

    /// The user's menu, which gets the mirrored entries.
    pub fn applications(&self) -> PathBuf {
        self.data_dir.join("applications")
    }

    /// The security-context socket handed to Waydroid as `WAYLAND_DISPLAY`.
    pub fn wayland_socket(&self) -> PathBuf {
        self.runtime_dir.join("crownos/android-wayland.sock")
    }

    /// Boot failures, warm-up and safe mode, kept across daemon runs.
    pub fn state_file(&self) -> PathBuf {
        self.home.join(".local/state/crownos/android.ron")
    }

    /// Where `diagnose` archives go.
    pub fn diagnose_dir(&self) -> PathBuf {
        self.home.join(".cache/crownos-android")
    }
}

/// Maps an inbox file name to the path the agent sees, refusing anything that
/// is not a plain file name.
pub fn guest_inbox_path(file_name: &str) -> Option<String> {
    let plain = !file_name.is_empty()
        && file_name != "."
        && file_name != ".."
        && !file_name.contains(['/', '\0']);
    plain.then(|| format!("/data/crownos/inbox/{file_name}"))
}

fn absolute(value: Option<OsString>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Paths {
        Paths::under_home(Path::new("/home/ada"), Path::new("/run/user/1000"))
    }

    #[test]
    fn the_session_sees_the_users_waydroid_directory_through_its_link() {
        let paths = paths();
        assert_eq!(
            paths.session_waydroid_link(),
            Path::new("/home/ada/.local/share/crownos/android/waydroid")
        );
        assert_eq!(
            paths.agent_socket(),
            Path::new("/home/ada/.local/share/waydroid/data/crownos/agent.sock")
        );
    }

    #[test]
    fn contract_paths_match() {
        let paths = paths();
        assert_eq!(
            paths.wayland_socket(),
            Path::new("/run/user/1000/crownos/android-wayland.sock")
        );
        assert_eq!(
            paths.state_file(),
            Path::new("/home/ada/.local/state/crownos/android.ron")
        );
        assert_eq!(
            paths.waydroid_applications(),
            Path::new("/home/ada/.local/share/crownos/android/applications")
        );
    }

    #[test]
    fn inbox_names_cannot_escape_the_inbox() {
        assert_eq!(
            guest_inbox_path("app.apk").as_deref(),
            Some("/data/crownos/inbox/app.apk")
        );
        assert_eq!(guest_inbox_path("../app.apk"), None);
        assert_eq!(guest_inbox_path(".."), None);
        assert_eq!(guest_inbox_path(""), None);
    }
}
