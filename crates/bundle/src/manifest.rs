use std::collections::BTreeMap;
use std::path::Path;

use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

use crate::util::sha256;
use crate::{BundleError, OtaEntry, OverlayPath};

/// Format version of [`Manifest`]. Readers reject newer formats.
pub const FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    /// LineageOS without Google services.
    Vanilla,
    /// LineageOS with Google Play.
    Gapps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    X86_64,
    Arm64,
}

/// The system and vendor images one flavor and architecture were tested with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagePin {
    pub flavor: Flavor,
    pub arch: Arch,
    pub system: OtaEntry,
    pub vendor: OtaEntry,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayFile {
    pub path: OverlayPath,
    pub sha256: String,
    pub mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    /// Reported by the HWC and the agent in their handshake.
    pub version: String,
    /// The exact `waydroid` package version the bundle was tested with.
    pub waydroid: String,
    pub images: Vec<ImagePin>,
    pub files: Vec<OverlayFile>,
    /// Overlay paths that hide a stock file, for example a stale `oat/` directory.
    pub whiteouts: Vec<OverlayPath>,
    /// Written to the `[properties]` section of `waydroid.cfg`.
    pub props: BTreeMap<String, String>,
}

impl Manifest {
    /// Parses `manifest` only after `signature` (minisign) verifies against `public_key`.
    pub fn verify(manifest: &[u8], signature: &str, public_key: &str) -> Result<Self, BundleError> {
        let key = PublicKey::from_base64(public_key)?;
        key.verify(manifest, &Signature::decode(signature)?, false)?;
        let parsed: Self = serde_json::from_slice(manifest)?;
        if parsed.format > FORMAT {
            return Err(BundleError::Format {
                found: parsed.format,
                supported: FORMAT,
            });
        }
        Ok(parsed)
    }

    pub fn pin(&self, flavor: Flavor, arch: Arch) -> Option<&ImagePin> {
        self.images
            .iter()
            .find(|pin| pin.flavor == flavor && pin.arch == arch)
    }

    /// Checks every listed file in the unpacked bundle at `root` against its sha256.
    pub fn check_files(&self, root: &Path) -> Result<(), BundleError> {
        self.files.iter().try_for_each(|file| {
            let path = file.path.under(root);
            let actual = sha256::of_file(&path).map_err(|source| BundleError::Read {
                path: path.display().to_string(),
                source,
            })?;
            if actual.eq_ignore_ascii_case(&file.sha256) {
                Ok(())
            } else {
                Err(BundleError::Checksum {
                    path: file.path.as_str().to_owned(),
                    expected: file.sha256.clone(),
                    actual,
                })
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = r#"{
  "format": 1,
  "version": "2026.11.0",
  "waydroid": "1.6.3",
  "images": [
    {
      "flavor": "gapps",
      "arch": "x86_64",
      "system": {
        "datetime": 1775208984,
        "filename": "lineage-20.0-20260403-GAPPS-waydroid_x86_64-system.zip",
        "id": "0000000000000000000000000000000000000000000000000000000000000000",
        "romtype": "GAPPS",
        "size": 1000,
        "url": "https://example.invalid/system.zip",
        "version": "20.0"
      },
      "vendor": {
        "datetime": 1777409843,
        "filename": "lineage-20.0-20260428-MAINLINE-waydroid_x86_64-vendor.zip",
        "id": "1111111111111111111111111111111111111111111111111111111111111111",
        "romtype": "MAINLINE",
        "size": 2000,
        "url": "https://example.invalid/vendor.zip",
        "version": "20.0"
      }
    }
  ],
  "files": [
    {
      "path": "vendor/lib64/hw/hwcomposer.waydroid.so",
      "sha256": "2222222222222222222222222222222222222222222222222222222222222222",
      "mode": 420
    }
  ],
  "whiteouts": [],
  "props": {
    "persist.waydroid.multi_windows": "true"
  }
}"#;

    #[test]
    fn reads_the_golden_manifest() -> Result<(), serde_json::Error> {
        let manifest: Manifest = serde_json::from_str(GOLDEN)?;
        assert_eq!(manifest.version, "2026.11.0");
        let pin = manifest.pin(Flavor::Gapps, Arch::X86_64);
        assert_eq!(pin.map(|pin| pin.system.datetime), Some(1775208984));
        assert_eq!(manifest.pin(Flavor::Vanilla, Arch::X86_64), None);
        assert_eq!(serde_json::to_string_pretty(&manifest)?, GOLDEN);
        Ok(())
    }

    #[test]
    fn a_manifest_with_an_escaping_path_does_not_parse() {
        let hostile = GOLDEN.replace("vendor/lib64/hw/hwcomposer.waydroid.so", "../../etc/passwd");
        assert!(serde_json::from_str::<Manifest>(&hostile).is_err());
    }

    #[test]
    fn an_unsigned_manifest_is_rejected() {
        let verified = Manifest::verify(
            GOLDEN.as_bytes(),
            "untrusted comment: none\nnot a signature",
            "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3",
        );
        assert!(matches!(verified, Err(BundleError::Signature(_))));
    }
}
