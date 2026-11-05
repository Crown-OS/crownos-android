//! Following the system's Android setup: the installation, `setup.json`, the
//! setup card and the menu entries.

use std::path::{Path, PathBuf};
use std::time::Instant;

use android_protocol::android;
use crownos_ipc::Server;
use runtime::installation::load_setup;
use runtime::paths::SETUP_DIR;
use runtime::{Installation, Paths};
use setup_state::Stage;

use crate::daemon::{Daemon, Topic};
use crate::notify::SetupNotice;

/// Where the installer writes `android.ron`.
const SYSTEM_CONFIG_DIR: &str = "/etc/crownos";

impl Daemon {
    /// Re-reads the installation and `setup.json` after either changed.
    pub fn reload_installation(&mut self, server: &mut Server, now: Instant) {
        self.follow_directories();
        let root = &self.env.system_root;
        let installation = Installation::load(root);
        let status = load_setup(root);
        if status != self.setup_status {
            if let Some(status) = &status {
                let event = android::SetupProgress {
                    status: status.clone(),
                };
                if let Err(error) = server.emit(&event) {
                    log::warn!("could not emit SetupProgress: {error}");
                }
            }
            self.setup_status = status.clone();
        }
        let setup_running = matches!(
            installation,
            Installation::SettingUp | Installation::Unsupported { .. }
        );
        if setup_running && let Some(status) = status.filter(|status| status.stage != Stage::Ready)
        {
            self.notifications.show_setup(SetupNotice::Progress(status));
        }
        self.update_menu(&installation);
        self.lifecycle.set_installation(installation, now);
    }

    /// Mirrors Waydroid's desktop entries after they changed.
    pub fn sync_apps(&mut self) {
        if self.menu_enabled != Some(true) {
            return;
        }
        let paths = &self.env.paths;
        match desktop_entries::sync(&paths.waydroid_applications(), &paths.applications()) {
            Ok(report) if report.changed() => log::info!(
                "menu: {} written, {} removed",
                report.written.len(),
                report.removed.len()
            ),
            Ok(_) => {}
            Err(error) => log::warn!("could not mirror the Android apps: {error}"),
        }
    }

    /// Watches every directory that exists now and is not watched yet.
    pub fn follow_directories(&mut self) {
        let paths = &self.env.paths;
        let apps = paths.waydroid_applications();
        if let Err(error) = std::fs::create_dir_all(&apps) {
            log::warn!("could not create {}: {error}", apps.display());
        }
        let wanted = [
            (Topic::Setup, self.system_path(SETUP_DIR)),
            (Topic::SystemConfig, self.system_path(SYSTEM_CONFIG_DIR)),
            (Topic::Apps, apps),
        ];
        for (topic, dir) in wanted {
            if self.watch.is_watching(topic) || !dir.is_dir() {
                continue;
            }
            if let Err(error) = self.watch.add(&dir, topic) {
                log::warn!("cannot follow {}: {error}", dir.display());
            }
        }
    }

    fn system_path(&self, path: &str) -> PathBuf {
        self.env.system_root.join(path.trim_start_matches('/'))
    }

    /// Android's entries belong in the menu while Android support is on.
    fn update_menu(&mut self, installation: &Installation) {
        let enabled = *installation != Installation::NotInstalled;
        if self.menu_enabled == Some(enabled) {
            return;
        }
        self.menu_enabled = Some(enabled);
        let target = self.env.paths.applications();
        let updated = if enabled {
            install_menu(&target)
        } else {
            desktop_entries::remove_all(&target)
        };
        if let Err(error) = updated {
            log::warn!("could not update the menu in {}: {error}", target.display());
        }
        if enabled {
            self.sync_apps();
        } else {
            self.notifications.close_setup();
        }
    }
}

fn install_menu(target: &Path) -> std::io::Result<()> {
    desktop_entries::install_handlers(target)?;
    desktop_entries::hide_waydroid_entries(target)
}

/// Whether `paths` names the Waydroid entry of a known package.
pub fn knows_package(paths: &Paths, package: &str) -> bool {
    paths
        .waydroid_applications()
        .join(desktop_entries::file_of_package(package))
        .exists()
}
