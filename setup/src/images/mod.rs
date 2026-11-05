//! The pinned Waydroid images: downloaded with resume into
//! `/var/lib/crownos/android/staging/`, verified, and unpacked into
//! `/var/lib/crownos/android/images/<system>-<vendor>/`. The verified zips
//! stay staged until `waydroid init` has run, because the channel it reads
//! points at them.

pub mod download;
pub mod extract;
pub mod network;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure};
use bundle::{ImagePin, OtaEntry};
use setup_state::{FailureCode, Stage};

use self::download::{Download, DownloadError};
use self::network::Backoff;
use crate::util::{atomic, name};
use crate::{FailWith, Paths, Retryable, SetupFailure, StatusSink};

/// The files `waydroid init -i` expects in an images directory.
pub const IMAGE_NAMES: [&str; 2] = ["system.img", "vendor.img"];

/// Failed attempts in a row, without new bytes, before setup gives up.
const ATTEMPTS_WITHOUT_PROGRESS: u32 = 8;
const PARTIAL_SUFFIX: &str = ".part";

/// The unpacked images of `pin`, downloading and unpacking what is missing.
pub fn ensure_unpacked(
    paths: &Paths,
    pin: &ImagePin,
    retry: Backoff,
    sink: &mut dyn StatusSink,
) -> Result<PathBuf, SetupFailure> {
    let images = paths
        .images()
        .join(format!("{}-{}", pin.system.datetime, pin.vendor.datetime));
    if IMAGE_NAMES.iter().all(|image| images.join(image).is_file()) {
        return Ok(images);
    }
    let entries = [&pin.system, &pin.vendor];
    for entry in entries {
        if !name::is_file_name(&entry.filename) {
            return Err(SetupFailure::new(
                FailureCode::Bundle,
                Retryable::No,
                anyhow!(
                    "the pinned file name {:?} is not a plain name",
                    entry.filename
                ),
            ));
        }
    }
    let staging = paths.staging();
    fs::create_dir_all(&staging).fail_with(FailureCode::Internal, Retryable::Yes)?;
    prune_staging(&staging, &entries).fail_with(FailureCode::Internal, Retryable::Yes)?;
    download_all(&staging, &entries, retry, sink)?;

    sink.stage(Stage::Verifying, None, "Unpacking Android")
        .fail_with(FailureCode::Internal, Retryable::Yes)?;
    let zips = entries.map(|entry| staging.join(&entry.filename));
    unpack(&zips, &images).fail_with(FailureCode::Internal, Retryable::Yes)?;
    Ok(images)
}

/// Where the verified zip of `entry` waits until `waydroid init` has run.
pub fn staged_zip(paths: &Paths, entry: &OtaEntry) -> PathBuf {
    paths.staging().join(&entry.filename)
}

/// Deletes the staged zips once Waydroid is initialised from the unpacked images.
pub fn discard_staged(paths: &Paths, pin: &ImagePin) -> io::Result<()> {
    for entry in [&pin.system, &pin.vendor] {
        match fs::remove_file(staged_zip(paths, entry)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

/// Removes staged files that belong to no current pin, such as the other
/// flavor's image after the Google Play choice changed.
fn prune_staging(staging: &Path, entries: &[&OtaEntry]) -> io::Result<()> {
    for found in fs::read_dir(staging)? {
        let found = found?;
        let file_name = found.file_name();
        let kept = entries.iter().any(|entry| {
            file_name.to_str().is_some_and(|name| {
                name.strip_suffix(PARTIAL_SUFFIX).unwrap_or(name) == entry.filename
            })
        });
        if kept {
            continue;
        }
        if found.file_type()?.is_dir() {
            fs::remove_dir_all(found.path())?;
        } else {
            fs::remove_file(found.path())?;
        }
    }
    Ok(())
}

fn download_all(
    staging: &Path,
    entries: &[&OtaEntry],
    retry: Backoff,
    sink: &mut dyn StatusSink,
) -> Result<(), SetupFailure> {
    let total: u64 = entries.iter().map(|entry| entry.size).sum();
    let mut finished = 0;
    let agent = download::agent();
    for entry in entries {
        let complete = staging.join(&entry.filename);
        if !complete.exists() {
            let partial = staging.join(format!("{}{PARTIAL_SUFFIX}", entry.filename));
            let download = Download {
                url: &entry.url,
                size: entry.size,
                sha256: &entry.id,
                partial: &partial,
                complete: &complete,
            };
            fetch_with_retries(&agent, &download, retry, sink, |bytes| {
                percent(finished + bytes, total)
            })?;
        }
        finished += entry.size;
    }
    Ok(())
}

fn fetch_with_retries(
    agent: &ureq::Agent,
    download: &Download,
    retry: Backoff,
    sink: &mut dyn StatusSink,
    overall: impl Fn(u64) -> u8,
) -> Result<(), SetupFailure> {
    let authority =
        network::authority(download.url).fail_with(FailureCode::Bundle, Retryable::No)?;
    let mut pauses = retry.pauses();
    let mut failures = 0;
    loop {
        network::wait_until_reachable(&authority, retry, || {
            sink.stage(Stage::WaitingForNetwork, None, "Waiting for the internet")
        })
        .fail_with(FailureCode::Internal, Retryable::Yes)?;
        let before = partial_len(download.partial);
        let mut report = |bytes| {
            if let Err(error) = sink.stage(
                Stage::Downloading,
                Some(overall(bytes)),
                "Downloading Android",
            ) {
                log::warn!("could not record the download progress: {error:#}");
            }
        };
        let error = match download::fetch(agent, download, &mut report) {
            Ok(()) => return Ok(()),
            Err(DownloadError::Interrupted(error)) => error,
            Err(error @ DownloadError::Refused(_)) => {
                return Err(SetupFailure::new(
                    FailureCode::Network,
                    Retryable::No,
                    error.into(),
                ));
            }
            Err(error @ DownloadError::Checksum { .. }) => {
                return Err(SetupFailure::new(
                    FailureCode::Checksum,
                    Retryable::Yes,
                    error.into(),
                ));
            }
            Err(error @ DownloadError::Disk(_)) => {
                return Err(SetupFailure::new(
                    FailureCode::Internal,
                    Retryable::Yes,
                    error.into(),
                ));
            }
        };
        failures = if partial_len(download.partial) > before {
            1
        } else {
            failures + 1
        };
        if failures >= ATTEMPTS_WITHOUT_PROGRESS {
            return Err(SetupFailure::new(
                FailureCode::Network,
                Retryable::Yes,
                error,
            ));
        }
        log::warn!("downloading {} failed, retrying: {error:#}", download.url);
        pauses.sleep();
    }
}

fn partial_len(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

fn percent(done: u64, total: u64) -> u8 {
    u8::try_from((done.saturating_mul(100) / total.max(1)).min(100)).unwrap_or(100)
}

/// Unpacks every zip into a hidden sibling of `images` and renames it into
/// place, so a complete images directory is the only one that ever exists.
fn unpack(zips: &[PathBuf], images: &Path) -> anyhow::Result<()> {
    let partial = atomic::sibling(images, "partial")?;
    for stale in [&partial, images] {
        if stale.exists() {
            fs::remove_dir_all(stale)?;
        }
    }
    fs::create_dir_all(&partial)?;
    for zip in zips {
        extract::unzip(zip, &partial)?;
    }
    for image in IMAGE_NAMES {
        ensure!(
            partial.join(image).is_file(),
            "the pinned zips hold no {image}"
        );
    }
    fs::rename(&partial, images)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_is_over_all_bytes_and_capped() {
        assert_eq!(percent(0, 200), 0);
        assert_eq!(percent(50, 200), 25);
        assert_eq!(percent(250, 200), 100);
        assert_eq!(percent(0, 0), 0);
    }
}
