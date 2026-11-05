use std::fs::{self, File, Permissions};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;

use anyhow::{Context, bail};
use bundle::OverlayPath;
use rustix::fs::{CWD, FileType, Mode};

use crate::Paths;
use crate::util::{atomic, sha256};

/// Mode of overlay files that come without one.
pub const FILE_MODE: u32 = 0o644;
const PERMISSION_BITS: u32 = 0o777;
const WHITEOUT_DEVICE: (u32, u32) = (0, 0);

/// Drops overlayfs's upper and work dirs, so no stale copy-up shadows the new
/// lower files. Waydroid recreates them at the next mount.
pub fn clear_upper_layers(paths: &Paths) -> io::Result<()> {
    for dir in [paths.overlay_rw(), paths.overlay_work()] {
        match fs::remove_dir_all(&dir) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

/// Copies `source` to `target` through a staged sibling, hashing on the way.
/// With `expected_sha256`, a mismatch leaves `target` untouched.
pub fn place(
    source: &Path,
    target: &Path,
    mode: u32,
    expected_sha256: Option<&str>,
) -> anyhow::Result<()> {
    let staged = atomic::sibling(target, "crownos-new")?;
    let mut input =
        File::open(source).with_context(|| format!("could not open {}", source.display()))?;
    let mut output = File::create(&staged)?;
    let actual = sha256::copy(&mut input, &mut output)?;
    if let Some(expected) = expected_sha256
        && !actual.eq_ignore_ascii_case(expected)
    {
        fs::remove_file(&staged)?;
        bail!(
            "{} has sha256 {actual}, the manifest says {expected}",
            source.display()
        );
    }
    output.set_permissions(Permissions::from_mode(mode & PERMISSION_BITS))?;
    output.sync_all()?;
    fs::rename(&staged, target)?;
    Ok(())
}

/// Puts an overlayfs whiteout, a 0:0 character device, at `path`.
pub fn place_whiteout(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_char_device() && metadata.rdev() == 0 => {
            return Ok(());
        }
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let (major, minor) = WHITEOUT_DEVICE;
    rustix::fs::mknodat(
        CWD,
        path,
        FileType::CharacterDevice,
        Mode::empty(),
        rustix::fs::makedev(major, minor),
    )?;
    Ok(())
}

/// Removes `listed` overlay paths, then any directory that leaves empty below
/// the `system/` and `vendor/` roots, which Waydroid's mounts need.
pub fn remove_listed<'a>(
    overlay_root: &Path,
    listed: impl Iterator<Item = &'a OverlayPath>,
) -> io::Result<()> {
    for path in listed {
        let target = path.under(overlay_root);
        match fs::remove_file(&target) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let below_root = target.ancestors().skip(1).take_while(|dir| {
            dir.strip_prefix(overlay_root)
                .is_ok_and(|inside| inside.components().count() > 1)
        });
        for dir in below_root {
            if fs::remove_dir(dir).is_err() {
                break;
            }
        }
    }
    Ok(())
}
