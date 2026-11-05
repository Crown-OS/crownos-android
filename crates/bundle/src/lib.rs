//! The CrownOS Android overlay bundle: a signed `manifest.json` that pins the
//! Waydroid package and images it was tested with, and lists every file it
//! places under `/var/lib/waydroid/overlay`.
//!
//! The setup service and `crown-androidd` trust nothing in a bundle until
//! [`Manifest::verify`] accepts its signature, and they only write the paths
//! [`OverlayPath`] allows.

mod channel;
mod manifest;
mod overlay_path;
mod util;

pub use channel::{OtaChannel, OtaEntry, Part};
pub use manifest::{Arch, FORMAT, Flavor, ImagePin, Manifest, OverlayFile};
pub use overlay_path::OverlayPath;

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("the manifest signature is not valid: {0}")]
    Signature(#[from] minisign_verify::Error),
    #[error("the manifest is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("manifest format {found} is newer than this reader ({supported})")]
    Format { found: u32, supported: u32 },
    #[error("overlay path {0:?} leaves the overlay or is not under system/ or vendor/")]
    UnsafePath(String),
    #[error("{path} has sha256 {actual}, the manifest says {expected}")]
    Checksum {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("could not read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
}
