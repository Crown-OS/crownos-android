use std::fs;
use std::io;
use std::path::Path;

use crate::VenusError;

/// The contents of a sysfs or procfs file without its trailing newline, or
/// `None` when the file does not exist.
pub fn read_value(path: &Path) -> Result<Option<String>, VenusError> {
    match fs::read_to_string(path) {
        Ok(mut value) => {
            value.truncate(value.trim_end().len());
            Ok(Some(value))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(read_error(path, source)),
    }
}

/// The UTF-8 entry names of `dir`, empty when it does not exist.
pub fn entry_names(dir: &Path) -> Result<Vec<String>, VenusError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(read_error(dir, source)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| read_error(dir, source))?;
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    Ok(names)
}

pub fn exists(path: &Path) -> Result<bool, VenusError> {
    path.try_exists().map_err(|source| read_error(path, source))
}

fn read_error(path: &Path, source: io::Error) -> VenusError {
    VenusError::Read {
        path: path.display().to_string(),
        source,
    }
}
