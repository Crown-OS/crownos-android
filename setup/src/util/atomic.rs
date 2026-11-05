use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Writes `bytes` to a sibling temp file, syncs it and renames it over `path`,
/// so a reader sees either the old or the new content.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = sibling(path, "new")?;
    let mut file = File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)
}

/// A hidden file next to `path` that ends in `.<tag>`, for staged replacements.
pub fn sibling(path: &Path, tag: &str) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    Ok(parent.join(format!(".{}.{tag}", name.to_string_lossy())))
}
