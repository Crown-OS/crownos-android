//! `~/.local/state/crownos/android.ron`: what the runtime must remember
//! between daemon runs.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::util::atomic;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Persisted {
    /// Boots in a row that never reached `Ready`.
    pub failed_boots: u32,
    /// Whether the first boot after setup ran.
    pub warmed_up: bool,
    /// The bundle version that was installed when safe mode was turned on.
    /// `Some` means Android has `persist.crownos.safe_mode` set.
    pub safe_mode_bundle: Option<String>,
    /// The last Google services framework id the agent reported.
    pub gsf_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("could not write {path}: {source}")]
    Write { path: String, source: io::Error },
    #[error("could not encode the runtime state: {0}")]
    Encode(#[from] ron::Error),
}

impl Persisted {
    /// The stored state, or the default when the file is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        ron::from_str(&text).unwrap_or_else(|error| {
            log::warn!("ignoring {}: {error}", path.display());
            Self::default()
        })
    }

    pub fn store(&self, path: &Path) -> Result<(), PersistError> {
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?;
        atomic::write(path, text.as_bytes()).map_err(|source| PersistError::Write {
            path: path.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_state_loads_back() -> Result<(), PersistError> {
        let dir = std::env::temp_dir().join(format!("runtime-persisted-{}", std::process::id()));
        let path = dir.join("crownos/android.ron");
        let state = Persisted {
            failed_boots: 2,
            warmed_up: true,
            safe_mode_bundle: Some("2026.11.0".into()),
            gsf_id: Some("3a1f".into()),
        };
        state.store(&path)?;
        assert_eq!(Persisted::load(&path), state);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn a_missing_file_is_a_fresh_state() {
        assert_eq!(
            Persisted::load(Path::new("/nonexistent/android.ron")),
            Persisted::default()
        );
    }
}
