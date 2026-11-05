//! `/var/lib/crownos/android/setup.json`: how far the root setup service has
//! got. `crownos-android-setup` is the only writer. `crown-androidd` follows the
//! file and turns it into the setup notification.
//!
//! New fields only ever go last, so an older reader still decodes a newer record
//! over `crownos-ipc` (postcard ignores trailing bytes).

mod status;
mod util;

pub use status::{Failure, FailureCode, SetupStatus, Stage};

use std::path::Path;

/// Present while setup still has work to do. The installer creates it and the
/// setup service removes it once Android is ready.
pub const PENDING_MARKER: &str = "/var/lib/crownos/android/pending";

/// Where [`SetupStatus`] lives.
pub const STATUS_PATH: &str = "/var/lib/crownos/android/setup.json";

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("could not read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("could not parse {path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
}

/// The current record, or `None` when setup never started.
pub fn load(path: &Path) -> Result<Option<SetupStatus>, StateError> {
    let text = match std::fs::read(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(read_error(path, source)),
    };
    serde_json::from_slice(&text)
        .map(Some)
        .map_err(|source| StateError::Parse {
            path: path.display().to_string(),
            source,
        })
}

/// Replaces the record in one step, so a reader never sees half of it.
pub fn store(path: &Path, status: &SetupStatus) -> Result<(), StateError> {
    let bytes = serde_json::to_vec_pretty(status).map_err(|source| StateError::Parse {
        path: path.display().to_string(),
        source,
    })?;
    util::atomic::write(path, &bytes).map_err(|source| StateError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn read_error(path: &Path, source: std::io::Error) -> StateError {
    StateError::Read {
        path: path.display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_means_setup_never_started() {
        let missing = Path::new("/nonexistent/crownos/android/setup.json");
        assert!(matches!(load(missing), Ok(None)));
    }

    #[test]
    fn a_stored_record_loads_back_unchanged() -> Result<(), StateError> {
        let dir = std::env::temp_dir().join(format!("setup-state-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|source| read_error(&dir, source))?;
        let path = dir.join("setup.json");
        let status = SetupStatus::new(Stage::Downloading, Some(42), "Downloading Android");
        store(&path, &status)?;
        assert_eq!(load(&path)?, Some(status));
        std::fs::remove_dir_all(&dir).map_err(|source| read_error(&dir, source))?;
        Ok(())
    }
}
