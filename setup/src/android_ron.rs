//! `/etc/crownos/android.ron` (contract 6): read through crownos-config,
//! written back atomically when setup records the tier and the bundle.

use std::path::Path;

use anyhow::Context;
use crownos_config::system::AndroidConfig;

use crate::util::atomic;

pub fn load(path: &Path) -> anyhow::Result<Option<AndroidConfig>> {
    AndroidConfig::load_from(path).with_context(|| format!("could not read {}", path.display()))
}

pub fn save(path: &Path, config: &AndroidConfig) -> anyhow::Result<()> {
    let mut text = config.to_ron()?;
    text.push('\n');
    atomic::write(path, text.as_bytes())
        .with_context(|| format!("could not write {}", path.display()))
}
