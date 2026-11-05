//! The signed overlay bundle that makes Android windows native: found under
//! `/usr/share/crownos-android/bundle/<version>/`, verified, and installed into
//! `/var/lib/waydroid/overlay`.

pub mod overlay;
pub mod record;
pub mod shipped;

pub use shipped::ShippedBundle;

use std::ffi::OsStr;

use bundle::{Manifest, OverlayPath};
use setup_state::FailureCode;

use crate::probe::Graphics;
use crate::waydroid::{PROPERTIES_SECTION, WAYDROID_SECTION, WaydroidConfig};
use crate::{FailWith, Host, Paths, Retryable, SetupFailure};

/// Waydroid silently turns overlays off when a mount fails once (plan risk 5).
const MOUNT_OVERLAYS: (&str, &str) = ("mount_overlays", "True");

/// Installs `bundle`: clears the overlay upper layers, places its files and
/// whiteouts (and the Venus files on that tier), merges its properties, runs
/// `waydroid upgrade --offline` and records the manifest. Files of the
/// `previous` bundle that this one no longer lists are removed.
pub fn install(
    paths: &Paths,
    host: &dyn Host,
    bundle: &ShippedBundle,
    graphics: &Graphics,
    previous: Option<&str>,
) -> Result<(), SetupFailure> {
    let manifest = &bundle.manifest;
    let overlay_root = paths.overlay();
    overlay::clear_upper_layers(paths).fail_with(FailureCode::Bundle, Retryable::Yes)?;
    if let Some(previous) = previous.filter(|previous| *previous != manifest.version)
        && let Some(old) =
            record::load(paths, previous).fail_with(FailureCode::Bundle, Retryable::Yes)?
    {
        let stale = listed(&old).filter(|path| !listed(manifest).any(|kept| kept == *path));
        overlay::remove_listed(&overlay_root, stale)
            .fail_with(FailureCode::Bundle, Retryable::Yes)?;
    }
    for file in &manifest.files {
        overlay::place(
            &file.path.under(&bundle.dir),
            &file.path.under(&overlay_root),
            file.mode,
            Some(&file.sha256),
        )
        .fail_with(FailureCode::Bundle, Retryable::No)?;
    }
    if graphics.venus().is_some() {
        for path in venus_files().fail_with(FailureCode::Bundle, Retryable::No)? {
            overlay::place(
                &path.under(&paths.venus_overlay()),
                &path.under(&overlay_root),
                overlay::FILE_MODE,
                None,
            )
            .fail_with(FailureCode::Bundle, Retryable::No)?;
        }
    }
    for whiteout in &manifest.whiteouts {
        overlay::place_whiteout(&whiteout.under(&overlay_root))
            .fail_with(FailureCode::Bundle, Retryable::Yes)?;
    }
    merge_props(paths, manifest, graphics).fail_with(FailureCode::Bundle, Retryable::No)?;
    host.waydroid(&[OsStr::new("upgrade"), OsStr::new("--offline")])
        .fail_with(FailureCode::Bundle, Retryable::Yes)?;
    record::save(paths, bundle).fail_with(FailureCode::Bundle, Retryable::Yes)
}

/// Writes the bundle's and the tier's properties into `waydroid.cfg`.
pub fn merge_props(paths: &Paths, manifest: &Manifest, graphics: &Graphics) -> anyhow::Result<()> {
    WaydroidConfig::edit(&paths.waydroid_cfg(), |config| {
        config.set(WAYDROID_SECTION, MOUNT_OVERLAYS.0, MOUNT_OVERLAYS.1)?;
        for (key, value) in &manifest.props {
            config.set(PROPERTIES_SECTION, key, value)?;
        }
        for (key, value) in graphics.props() {
            config.set(PROPERTIES_SECTION, key, &value)?;
        }
        Ok(())
    })
}

/// Every overlay path a manifest owns: its files and its whiteouts.
pub fn listed(manifest: &Manifest) -> impl Iterator<Item = &OverlayPath> {
    manifest
        .files
        .iter()
        .map(|file| &file.path)
        .chain(&manifest.whiteouts)
}

/// The Venus overlay files, as validated overlay paths.
pub fn venus_files() -> Result<Vec<OverlayPath>, bundle::BundleError> {
    venus::overlay_files()
        .iter()
        .map(|path| OverlayPath::try_from((*path).to_owned()))
        .collect()
}
