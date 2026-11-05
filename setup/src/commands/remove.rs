//! `crownos-android-setup remove [--wipe-data]`, run by Settings through
//! pkexec. It deletes only what setup created, plus, on request, each user's
//! Waydroid data.

use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context;
use crownos_config::system::{AndroidConfig, AndroidTier};

use super::{CONTAINER_UNIT, SETUP_UNIT};
use crate::bundle_install::{self, overlay, record};
use crate::util::{nofollow, passwd};
use crate::{Host, Paths, android_ron};

/// Whether `remove` also deletes every user's `~/.local/share/waydroid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserData {
    Keep,
    Wipe,
}

pub fn remove(paths: &Paths, host: &dyn Host, user_data: UserData) -> anyhow::Result<()> {
    host.systemctl(&["stop", CONTAINER_UNIT])
        .context("could not stop Waydroid")?;
    if let Err(error) = host.systemctl(&["disable", SETUP_UNIT]) {
        log::warn!("could not disable {SETUP_UNIT}: {error:#}");
    }
    let config = android_ron::load(&paths.android_ron())?;
    remove_overlay_files(paths, config.as_ref())?;
    for dir in [paths.images(), paths.staging(), paths.channel()] {
        remove_tree(&dir).with_context(|| format!("could not remove {}", dir.display()))?;
    }
    if let Some(mut config) = config {
        config.enabled = false;
        android_ron::save(&paths.android_ron(), &config)?;
    }
    remove_file(&paths.pending_marker())?;
    if user_data == UserData::Wipe {
        wipe_user_data(paths)?;
    }
    Ok(())
}

fn remove_overlay_files(paths: &Paths, config: Option<&AndroidConfig>) -> anyhow::Result<()> {
    let overlay_root = paths.overlay();
    let installed = config.and_then(|config| config.bundle.as_deref());
    if let Some(manifest) = installed
        .map(|version| record::load(paths, version))
        .transpose()?
        .flatten()
    {
        overlay::remove_listed(&overlay_root, bundle_install::listed(&manifest))?;
    }
    if config.is_some_and(|config| config.tier == Some(AndroidTier::Venus)) {
        overlay::remove_listed(&overlay_root, bundle_install::venus_files()?.iter())?;
    }
    Ok(())
}

fn wipe_user_data(paths: &Paths) -> anyhow::Result<()> {
    let passwd_text = fs::read_to_string(paths.passwd())?;
    for home in passwd::user_homes(&passwd_text) {
        match nofollow::open_dir(&paths.home(home), &[".local", "share"]) {
            Ok(share) => remove_tree(&nofollow::child(&share, "waydroid"))
                .with_context(|| format!("could not remove {home}/.local/share/waydroid"))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("left {home}/.local/share alone: {error}"),
        }
    }
    Ok(())
}

/// Removes `path` without following a symlink at its end.
fn remove_tree(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
