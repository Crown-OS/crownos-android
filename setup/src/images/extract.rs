use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

use anyhow::Context;
use zip::ZipArchive;

const WRITE_BUFFER: usize = 1 << 20;

/// Unpacks the files of a Waydroid image zip, which are all at its top level,
/// into `into`. Any nested or escaping entry is refused.
pub fn unzip(archive: &Path, into: &Path) -> anyhow::Result<()> {
    let mut zip = ZipArchive::new(File::open(archive)?)
        .with_context(|| format!("{} is not a zip", archive.display()))?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .enclosed_name()
            .filter(|name| name.components().count() == 1)
            .with_context(|| {
                format!(
                    "{} holds an unexpected entry {:?}",
                    archive.display(),
                    entry.name()
                )
            })?;
        let mut output = BufWriter::with_capacity(WRITE_BUFFER, File::create(into.join(name))?);
        io::copy(&mut entry, &mut output)?;
        output
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
    }
    Ok(())
}
