//! Fakes shared by the integration tests: a minisign signer, a Range-capable
//! HTTP server, a fake machine tree and a fake `waydroid`/`systemctl`.

#![allow(dead_code)]

pub mod host;
pub mod http;
pub mod machine;
pub mod sign;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the temp dir, removed on drop.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(label: &str) -> io::Result<Self> {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "crownos-android-setup-it-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deterministic pseudo-random bytes, so payloads do not compress to nothing.
pub fn payload(seed: u8, length: usize) -> Vec<u8> {
    let mut state = u32::from(seed).wrapping_mul(2_654_435_761) | 1;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state.to_le_bytes()[0]
        })
        .collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// A zip holding `entries`, stored uncompressed like Waydroid's images.
pub fn zip(entries: &[(&str, &[u8])]) -> zip::result::ZipResult<Vec<u8>> {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        writer.start_file(*name, options)?;
        writer.write_all(bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}
