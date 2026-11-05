use std::io;

use anyhow::Context;
use bundle::Manifest;

use super::shipped::{MANIFEST, SIGNATURE, ShippedBundle};
use crate::Paths;
use crate::util::{atomic, name};

/// Keeps a copy of an installed bundle's signed manifest under
/// `/var/lib/crownos/android/bundles/<version>/`.
pub fn save(paths: &Paths, bundle: &ShippedBundle) -> anyhow::Result<()> {
    let dir = paths.installed_bundles().join(&bundle.manifest.version);
    atomic::write(&dir.join(MANIFEST), &bundle.manifest_bytes)?;
    atomic::write(&dir.join(SIGNATURE), bundle.signature.as_bytes())?;
    Ok(())
}

/// The manifest of installed bundle `version`, if setup recorded one. Only
/// root writes the record, after verifying it, so it is parsed without the
/// key; parsing still rejects every path outside `system/` and `vendor/`.
pub fn load(paths: &Paths, version: &str) -> anyhow::Result<Option<Manifest>> {
    if !name::is_file_name(version) {
        return Ok(None);
    }
    let path = paths.installed_bundles().join(version).join(MANIFEST);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()));
        }
    };
    Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
        format!("could not parse {}", path.display())
    })?))
}
