use std::path::{Path, PathBuf};

use crownos_config::system::ANDROID_CONFIG_PATH;

const STATE_DIR: &str = "/var/lib/crownos/android";
const WAYDROID_DIR: &str = "/var/lib/waydroid";
const SHIPPED_BUNDLES: &str = "/usr/share/crownos-android/bundle";
const DISK_CHECKED: &str = "/var/lib";
const PASSWD: &str = "/etc/passwd";

/// Every location the setup service touches, all below one root: `/` on a
/// real system, a temporary directory in tests.
#[derive(Clone, Debug)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn system() -> Self {
        Self::under(Path::new("/"))
    }

    pub fn under(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    fn rooted(&self, absolute: &str) -> PathBuf {
        self.root.join(absolute.trim_start_matches('/'))
    }

    /// The filesystem root that program paths such as `usr/bin/ufw` live under.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn android_ron(&self) -> PathBuf {
        self.rooted(ANDROID_CONFIG_PATH)
    }

    pub fn pending_marker(&self) -> PathBuf {
        self.rooted(setup_state::PENDING_MARKER)
    }

    pub fn status(&self) -> PathBuf {
        self.rooted(setup_state::STATUS_PATH)
    }

    pub fn staging(&self) -> PathBuf {
        self.rooted(STATE_DIR).join("staging")
    }

    pub fn images(&self) -> PathBuf {
        self.rooted(STATE_DIR).join("images")
    }

    pub fn channel(&self) -> PathBuf {
        self.rooted(STATE_DIR).join("channel")
    }

    /// Copies of the manifests of installed bundles, kept so `remove` and the
    /// next bundle know which overlay files are ours.
    pub fn installed_bundles(&self) -> PathBuf {
        self.rooted(STATE_DIR).join("bundles")
    }

    pub fn shipped_bundles(&self) -> PathBuf {
        self.rooted(SHIPPED_BUNDLES)
    }

    pub fn venus_overlay(&self) -> PathBuf {
        self.rooted(venus::OVERLAY_SOURCE)
    }

    pub fn disk_checked(&self) -> PathBuf {
        self.rooted(DISK_CHECKED)
    }

    pub fn waydroid_cfg(&self) -> PathBuf {
        self.rooted(WAYDROID_DIR).join("waydroid.cfg")
    }

    pub fn overlay(&self) -> PathBuf {
        self.rooted(WAYDROID_DIR).join("overlay")
    }

    pub fn overlay_rw(&self) -> PathBuf {
        self.rooted(WAYDROID_DIR).join("overlay_rw")
    }

    pub fn overlay_work(&self) -> PathBuf {
        self.rooted(WAYDROID_DIR).join("overlay_work")
    }

    pub fn config_nodes(&self) -> PathBuf {
        self.rooted(WAYDROID_DIR).join("lxc/waydroid/config_nodes")
    }

    pub fn sys(&self) -> PathBuf {
        self.rooted("/sys")
    }

    pub fn proc_root(&self) -> PathBuf {
        self.rooted("/proc")
    }

    pub fn dev(&self) -> PathBuf {
        self.rooted("/dev")
    }

    pub fn passwd(&self) -> PathBuf {
        self.rooted(PASSWD)
    }

    /// A home directory from `/etc/passwd`, which names it relative to the real root.
    pub fn home(&self, home: &str) -> PathBuf {
        self.rooted(home)
    }
}
