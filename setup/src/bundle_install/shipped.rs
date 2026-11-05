use std::fs;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use bundle::{BundleError, Manifest};
use setup_state::FailureCode;

use crate::util::{name, version};
use crate::{FailWith, Retryable, SetupFailure};

pub const MANIFEST: &str = "manifest.json";
pub const SIGNATURE: &str = "manifest.json.minisig";

/// A bundle tree whose manifest verified against the CrownOS key.
#[derive(Debug)]
pub struct ShippedBundle {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub manifest_bytes: Vec<u8>,
    pub signature: String,
}

/// The newest bundle the package ships under `root`.
pub fn newest(root: &Path, public_key: &str) -> Result<ShippedBundle, SetupFailure> {
    let newest = fs::read_dir(root)
        .fail_with(FailureCode::Bundle, Retryable::No)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .file_type()
                .ok()?
                .is_dir()
                .then(|| entry.file_name().into_string().ok())?
        })
        .max_by(|left, right| version::compare(left, right))
        .ok_or_else(|| {
            SetupFailure::new(
                FailureCode::Bundle,
                Retryable::No,
                anyhow!("no bundle under {}", root.display()),
            )
        })?;
    load(&root.join(newest), public_key)
}

/// Reads and verifies the bundle in `dir`, whose name must be its version.
pub fn load(dir: &Path, public_key: &str) -> Result<ShippedBundle, SetupFailure> {
    let manifest_bytes =
        fs::read(dir.join(MANIFEST)).fail_with(FailureCode::Bundle, Retryable::No)?;
    let signature =
        fs::read_to_string(dir.join(SIGNATURE)).fail_with(FailureCode::Signature, Retryable::No)?;
    let manifest = Manifest::verify(&manifest_bytes, &signature, public_key).map_err(|error| {
        let code = match error {
            BundleError::Signature(_) => FailureCode::Signature,
            _ => FailureCode::Bundle,
        };
        SetupFailure::new(code, Retryable::No, error.into())
    })?;
    let named_for_version = dir
        .file_name()
        .is_some_and(|dir_name| *dir_name == *manifest.version);
    if !name::is_file_name(&manifest.version) || !named_for_version {
        return Err(SetupFailure::new(
            FailureCode::Bundle,
            Retryable::No,
            anyhow!(
                "{} holds bundle version {:?}",
                dir.display(),
                manifest.version
            ),
        ));
    }
    Ok(ShippedBundle {
        dir: dir.to_path_buf(),
        manifest,
        manifest_bytes,
        signature,
    })
}
