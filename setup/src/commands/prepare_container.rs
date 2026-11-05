//! `crownos-android-setup prepare-container`, the `ExecStartPre=` of
//! `waydroid-container.service`: a few file reads when nothing changed.

use std::io;

use crownos_config::system::AndroidTier;
use venus::Support;

use crate::waydroid::config_nodes;
use crate::{Paths, android_ron, binder, venus_socket};

/// Pins the binder nodes and, on the Venus tier, the NVIDIA render node in
/// the container's LXC config and each user's socket directory. binderfs is
/// checked in this process's own mount namespace, since the one `run` mounted
/// stayed in the setup unit's private namespace.
pub fn prepare_container(paths: &Paths) -> anyhow::Result<()> {
    binder::prepare_nodes(paths)?;
    binder::pin_names(&paths.waydroid_cfg())?;
    let tier = android_ron::load(&paths.android_ron())?.and_then(|config| config.tier);
    if tier != Some(AndroidTier::Venus) {
        sync_config_nodes(paths, None)?;
        return Ok(());
    }
    let support = venus::detect(&paths.sys(), &paths.proc_root())?;
    if support.is_none() {
        log::warn!("the Venus tier is configured, but this machine no longer supports it");
    }
    sync_config_nodes(paths, support.as_ref())?;
    venus_socket::prepare(paths)
}

/// Rewrites `config_nodes` with the canonical binder lines and the Venus line.
pub fn sync_config_nodes(paths: &Paths, venus: Option<&Support>) -> io::Result<bool> {
    let venus_line = venus.map(venus::container_mount_line);
    config_nodes::update(&paths.config_nodes(), venus_line.as_deref())
}
