use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::anyhow;
use sha2::{Digest, Sha256};
use ureq::Agent;

use crate::util::sha256;

const CHUNK: usize = 256 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
const USER_AGENT: &str = concat!("crownos-android-setup/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// Worth retrying when the network is back; the partial file is kept.
    #[error("the transfer was interrupted: {0:#}")]
    Interrupted(anyhow::Error),
    /// Retrying cannot help: the server does not have the pinned file.
    #[error("the server refused the download: {0:#}")]
    Refused(anyhow::Error),
    /// The complete file is not the pinned one; it has been deleted.
    #[error("the download has sha256 {actual}, the pin says {expected}")]
    Checksum { expected: String, actual: String },
    #[error("could not write the download: {0}")]
    Disk(#[from] io::Error),
}

/// One pinned file and where it is staged.
pub struct Download<'a> {
    pub url: &'a str,
    pub size: u64,
    pub sha256: &'a str,
    /// Grows across attempts and reboots until it holds `size` bytes.
    pub partial: &'a Path,
    /// Where the verified file lands.
    pub complete: &'a Path,
}

/// The HTTP client: rustls with webpki roots, redirects followed, statuses
/// handled here, and a stall timeout instead of a total one.
pub fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .timeout_per_read(Some(STALL_TIMEOUT))
        .build()
        .into()
}

/// Fetches `download.url` into `download.partial`, resuming with an HTTP
/// Range from the bytes already there, checks the sha256 and moves the file to
/// `download.complete`. `progress` gets the bytes on disk after each chunk.
pub fn fetch(
    agent: &Agent,
    download: &Download,
    progress: &mut dyn FnMut(u64),
) -> Result<(), DownloadError> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(download.partial)?;
    let mut offset = file.metadata()?.len();
    if offset > download.size {
        file.set_len(0)?;
        offset = 0;
    }
    let mut hasher = Sha256::new();
    sha256::update_from(&mut hasher, &mut (&file).take(offset))?;
    progress(offset);

    if offset < download.size {
        let mut request = agent.get(download.url);
        if offset > 0 {
            request = request.header("Range", format!("bytes={offset}-"));
        }
        let response = request
            .call()
            .map_err(|error| DownloadError::Interrupted(error.into()))?;
        match response.status().as_u16() {
            206 if resumes_at(response.headers(), offset) => {}
            206 => {
                file.set_len(0)?;
                return Err(DownloadError::Interrupted(anyhow!(
                    "the server resumed at another offset"
                )));
            }
            200 => {
                if offset > 0 {
                    file.set_len(0)?;
                    hasher = Sha256::new();
                    offset = 0;
                }
            }
            416 => {
                file.set_len(0)?;
                return Err(DownloadError::Interrupted(anyhow!(
                    "the server refused to resume at {offset}"
                )));
            }
            status @ (408 | 429 | 500..=599) => {
                return Err(DownloadError::Interrupted(anyhow!("HTTP {status}")));
            }
            status => {
                return Err(DownloadError::Refused(anyhow!(
                    "HTTP {status} for {}",
                    download.url
                )));
            }
        }
        let mut body = response.into_body().into_reader();
        offset = copy_body(
            &mut body,
            &mut file,
            &mut hasher,
            offset,
            download.size,
            progress,
        )?;
        if offset < download.size {
            return Err(DownloadError::Interrupted(anyhow!(
                "the connection closed after {offset} of {} bytes",
                download.size
            )));
        }
    }

    file.sync_all()?;
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(download.sha256) {
        drop(file);
        fs::remove_file(download.partial)?;
        return Err(DownloadError::Checksum {
            expected: download.sha256.to_owned(),
            actual,
        });
    }
    fs::rename(download.partial, download.complete)?;
    Ok(())
}

fn copy_body(
    body: &mut impl Read,
    file: &mut File,
    hasher: &mut Sha256,
    mut offset: u64,
    size: u64,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, DownloadError> {
    let mut chunk = vec![0; CHUNK];
    while offset < size {
        let read = match body.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(DownloadError::Interrupted(error.into())),
        };
        if offset + read as u64 > size {
            return Err(DownloadError::Refused(anyhow!(
                "the server sent more than the pinned {size} bytes"
            )));
        }
        file.write_all(&chunk[..read])?;
        hasher.update(&chunk[..read]);
        offset += read as u64;
        progress(offset);
    }
    Ok(offset)
}

/// Whether `Content-Range: bytes <start>-<end>/<total>` starts at `offset`.
fn resumes_at(headers: &ureq::http::HeaderMap, offset: u64) -> bool {
    headers
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes "))
        .and_then(|range| range.split_once('-'))
        .and_then(|(start, _)| start.trim().parse::<u64>().ok())
        == Some(offset)
}
