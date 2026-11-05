//! The session's private `XDG_DATA_HOME`: Waydroid writes its desktop entries
//! there instead of the user's menu, and reaches the user's real Waydroid
//! directory through a symlink.

use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

use crate::Paths;

const PRIVATE: u32 = 0o700;

pub fn prepare(paths: &Paths) -> io::Result<()> {
    let private = |dir: &Path| DirBuilder::new().recursive(true).mode(PRIVATE).create(dir);
    private(&paths.waydroid_applications())?;
    private(&paths.waydroid_state())?;
    link(&paths.session_waydroid_link(), &paths.waydroid_state())
}

/// Points `link` at `target`, replacing a stale symlink but never a real
/// directory, which would hold someone's Android data.
fn link(link: &Path, target: &Path) -> io::Result<()> {
    match fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if fs::read_link(link)? == target {
                return Ok(());
            }
            fs::remove_file(link)?;
        }
        Ok(_) => {
            log::warn!(
                "{} is not a symlink; leaving it, the session uses it as is",
                link.display()
            );
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::os::unix::fs::symlink(target, link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_the_session_to_the_users_waydroid_directory() -> io::Result<()> {
        let home = std::env::temp_dir().join(format!("runtime-data-home-{}", std::process::id()));
        let paths = Paths::under_home(&home, &home.join("run"));
        prepare(&paths)?;
        prepare(&paths)?;
        assert_eq!(
            fs::read_link(paths.session_waydroid_link())?,
            paths.waydroid_state()
        );
        assert!(paths.waydroid_applications().is_dir());
        fs::remove_dir_all(&home)
    }

    #[test]
    fn replaces_a_stale_link() -> io::Result<()> {
        let home = std::env::temp_dir().join(format!("runtime-stale-link-{}", std::process::id()));
        let paths = Paths::under_home(&home, &home.join("run"));
        fs::create_dir_all(paths.session_data_home())?;
        std::os::unix::fs::symlink("/nonexistent", paths.session_waydroid_link())?;
        prepare(&paths)?;
        assert_eq!(
            fs::read_link(paths.session_waydroid_link())?,
            paths.waydroid_state()
        );
        fs::remove_dir_all(&home)
    }
}
