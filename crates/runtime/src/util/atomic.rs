use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to a sibling temp file and renames it over `path`, so a
/// reader never sees half a file.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));
    let mut file = File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)
}
