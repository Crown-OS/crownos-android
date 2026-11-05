use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::BundleError;

/// A path relative to the overlay root that stays inside `system/` or `vendor/`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OverlayPath(String);

const ROOTS: [&str; 2] = ["system", "vendor"];

impl OverlayPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Where the path lands under `overlay_root`.
    pub fn under(&self, overlay_root: &Path) -> PathBuf {
        overlay_root.join(&self.0)
    }
}

impl TryFrom<String> for OverlayPath {
    type Error = BundleError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        let mut components = Path::new(&path).components();
        let rooted = matches!(
            components.next(),
            Some(Component::Normal(first)) if ROOTS.iter().any(|root| first == *root)
        );
        let plain = components.all(|component| matches!(component, Component::Normal(_)));
        if rooted && plain && !path.ends_with('/') {
            Ok(Self(path))
        } else {
            Err(BundleError::UnsafePath(path))
        }
    }
}

impl From<OverlayPath> for String {
    fn from(path: OverlayPath) -> Self {
        path.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(path: &str) -> Result<OverlayPath, BundleError> {
        OverlayPath::try_from(path.to_owned())
    }

    #[test]
    fn accepts_paths_inside_system_and_vendor() {
        assert!(parse("vendor/lib64/hw/hwcomposer.waydroid.so").is_ok());
        assert!(parse("system/etc/init/crown-agent.rc").is_ok());
    }

    #[test]
    fn rejects_paths_that_escape_or_sit_elsewhere() {
        for path in [
            "../etc/passwd",
            "/vendor/lib64/x.so",
            "vendor/../../etc/shadow",
            "data/local/tmp/x",
            "vendor/lib64/",
            "",
        ] {
            assert!(parse(path).is_err(), "{path} must be rejected");
        }
    }
}
