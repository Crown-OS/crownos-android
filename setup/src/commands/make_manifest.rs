//! `crownos-android-setup make-manifest --root <dir> --version <v> --pins <json>`:
//! writes `manifest.json` for an unpacked bundle tree in the CI build.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail, ensure};
use bundle::{ImagePin, Manifest, OverlayFile, OverlayPath};
use serde::{Deserialize, Serialize};

use crate::bundle_install::shipped;
use crate::props;
use crate::util::{atomic, name, sha256};

/// `overlay/pins.json`: the Waydroid package and the images a bundle is tested with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pins {
    pub waydroid: String,
    pub images: Vec<ImagePin>,
}

/// Overlay paths to hide, one per line, `#` starting a comment.
pub const WHITEOUTS: &str = "whiteouts.txt";
const ROOTS: [&str; 2] = ["system", "vendor"];
const PERMISSION_BITS: u32 = 0o777;

/// Writes `<root>/manifest.json` and returns its path.
pub fn make_manifest(root: &Path, version: &str, pins: &Path) -> anyhow::Result<PathBuf> {
    ensure!(
        name::is_file_name(version),
        "{version:?} cannot name a bundle directory"
    );
    let pins: Pins = serde_json::from_slice(
        &fs::read(pins).with_context(|| format!("could not read {}", pins.display()))?,
    )
    .with_context(|| format!("could not parse {}", pins.display()))?;
    let mut files = Vec::new();
    for top in ROOTS {
        collect(root, &root.join(top), &mut files)?;
    }
    files.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    let whiteouts = read_whiteouts(&root.join(WHITEOUTS))?;
    if let Some(clash) = whiteouts
        .iter()
        .find(|whiteout| files.iter().any(|file| file.path == **whiteout))
    {
        bail!("{} is both a file and a whiteout", clash.as_str());
    }
    let manifest = Manifest {
        format: bundle::FORMAT,
        version: version.to_owned(),
        waydroid: pins.waydroid,
        images: pins.images,
        files,
        whiteouts,
        props: props::bundle_props(version),
    };
    let mut json = serde_json::to_vec_pretty(&manifest)?;
    json.push(b'\n');
    let path = root.join(shipped::MANIFEST);
    atomic::write(&path, &json)?;
    Ok(path)
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<OverlayFile>) -> anyhow::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect(root, &path, files)?;
            continue;
        }
        if !file_type.is_file() {
            bail!(
                "{} is not a regular file; bundles hold regular files only",
                path.display()
            );
        }
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .with_context(|| format!("{} is not UTF-8", path.display()))?;
        files.push(OverlayFile {
            path: OverlayPath::try_from(relative.to_owned())?,
            sha256: sha256::of_file(&path)?,
            mode: entry.metadata()?.permissions().mode() & PERMISSION_BITS,
        });
    }
    Ok(())
}

fn read_whiteouts(path: &Path) -> anyhow::Result<Vec<OverlayPath>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    text.lines()
        .enumerate()
        .map(|(index, line)| (index, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(index, line)| {
            OverlayPath::try_from(line.to_owned())
                .with_context(|| format!("{}:{}: not an overlay path", path.display(), index + 1))
        })
        .collect()
}
