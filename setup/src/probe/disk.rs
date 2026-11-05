use std::io;
use std::path::Path;

/// Free space Android needs on `/var/lib`: both images unpacked plus the
/// zips they come from.
pub const MIN_FREE_BYTES: u64 = 6 << 30;

/// Bytes an unprivileged writer may still use on the filesystem holding `path`.
pub fn free_bytes(path: &Path) -> io::Result<u64> {
    let stats = rustix::fs::statvfs(path)?;
    Ok(stats.f_bavail.saturating_mul(stats.f_frsize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_temp_dir_reports_free_space() -> io::Result<()> {
        assert!(free_bytes(&std::env::temp_dir())? > 0);
        Ok(())
    }
}
