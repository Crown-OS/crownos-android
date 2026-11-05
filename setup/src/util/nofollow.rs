use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};

use rustix::fs::{Mode, OFlags};

const DIRECTORY: OFlags = OFlags::DIRECTORY
    .union(OFlags::RDONLY)
    .union(OFlags::CLOEXEC);

/// Opens `base`, then each of `components` below it without following a
/// symlink, so a user who owns those directories cannot redirect root
/// elsewhere.
pub fn open_dir(base: &Path, components: &[&str]) -> io::Result<OwnedFd> {
    let mut dir = rustix::fs::open(base, DIRECTORY, Mode::empty())?;
    for component in components {
        dir = rustix::fs::openat(
            &dir,
            *component,
            DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )?;
    }
    Ok(dir)
}

/// The path of `name` inside the already opened `dir`, which the kernel
/// resolves through the descriptor instead of walking the parents again.
pub fn child(dir: &OwnedFd, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{name}", dir.as_raw_fd()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scratch::Scratch;

    #[test]
    fn a_symlinked_component_is_refused() -> io::Result<()> {
        let base = Scratch::new("nofollow")?;
        std::fs::create_dir_all(base.join("real/share"))?;
        std::os::unix::fs::symlink(base.join("real"), base.join("link"))?;
        assert!(open_dir(&base, &["real", "share"]).is_ok());
        assert!(open_dir(&base, &["link", "share"]).is_err());
        Ok(())
    }
}
