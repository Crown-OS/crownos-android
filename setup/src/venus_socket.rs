//! The directory of the Venus vtest socket inside each user's Android data,
//! `~/.local/share/waydroid/data/crownos/venus`. Android owns `data` when the
//! desktop user is not uid 1000, so the render server cannot create it; root
//! does, here, for every user whose Android data exists.

use std::fs;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rustix::fs::{Gid, Mode, OFlags, Uid};

use crate::Paths;
use crate::util::{nofollow, passwd};

const ANDROID_DATA: [&str; 4] = [".local", "share", "waydroid", "data"];
const SOCKET_DIRS: [&str; 2] = ["crownos", "venus"];
const SOCKET_DIR_MODE: u32 = 0o711;

/// Creates `data/crownos` and `data/crownos/venus`, mode 0711 and owned by
/// the owner of the home directory, for every regular user. Nothing else is
/// chowned, and no symlink below the home directory is followed.
pub fn prepare(paths: &Paths) -> anyhow::Result<()> {
    let passwd_text = fs::read_to_string(paths.passwd())?;
    for home in passwd::user_homes(&passwd_text) {
        if let Err(error) = prepare_home(&paths.home(home)) {
            log::warn!("could not prepare the Venus socket directory of {home}: {error}");
        }
    }
    Ok(())
}

fn prepare_home(home: &Path) -> io::Result<()> {
    let owner = fs::metadata(home)?;
    let mut dir = match nofollow::open_dir(home, &ANDROID_DATA) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for name in SOCKET_DIRS {
        dir = owned_subdir(&dir, name, owner.uid(), owner.gid())?;
    }
    Ok(())
}

fn owned_subdir(parent: &OwnedFd, name: &str, uid: u32, gid: u32) -> io::Result<OwnedFd> {
    let mode = Mode::from_raw_mode(SOCKET_DIR_MODE);
    match rustix::fs::mkdirat(parent, name, mode) {
        Err(error) if error != rustix::io::Errno::EXIST => return Err(error.into()),
        _ => {}
    }
    let dir = rustix::fs::openat(
        parent,
        name,
        OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    rustix::fs::fchown(&dir, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))?;
    rustix::fs::fchmod(&dir, mode)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::util::scratch::Scratch;

    fn mode(path: &Path) -> io::Result<u32> {
        Ok(fs::metadata(path)?.permissions().mode() & 0o777)
    }

    #[test]
    fn socket_dirs_appear_for_users_with_android_data_only() -> anyhow::Result<()> {
        let root = Scratch::new("venus-socket")?;
        let paths = Paths::under(&root);
        let alice = root.join("home/alice");
        let data = alice.join(".local/share/waydroid/data");
        fs::create_dir_all(&data)?;
        fs::create_dir_all(data.join("crownos"))?;
        fs::set_permissions(data.join("crownos"), fs::Permissions::from_mode(0o700))?;
        fs::create_dir_all(root.join("home/bob"))?;
        let elsewhere = root.join("srv/target");
        fs::create_dir_all(elsewhere.join("share/waydroid/data"))?;
        let mallory = root.join("home/mallory");
        fs::create_dir_all(&mallory)?;
        std::os::unix::fs::symlink(&elsewhere, mallory.join(".local"))?;
        fs::create_dir_all(root.join("etc"))?;
        fs::write(
            paths.passwd(),
            "alice:x:1000:1000::/home/alice:/bin/fish\nbob:x:1001:1001::/home/bob:/bin/sh\nmallory:x:1002:1002::/home/mallory:/bin/sh\n",
        )?;

        for _ in 0..2 {
            prepare(&paths)?;
            let venus = data.join("crownos/venus");
            assert_eq!(mode(&data.join("crownos"))?, SOCKET_DIR_MODE);
            assert_eq!(mode(&venus)?, SOCKET_DIR_MODE);
            assert_eq!(fs::metadata(&venus)?.uid(), fs::metadata(&alice)?.uid());
        }
        assert!(!root.join("home/bob/.local").exists());
        assert!(!elsewhere.join("share/waydroid/data/crownos").exists());
        assert_ne!(
            mode(&data)?,
            SOCKET_DIR_MODE,
            "the data directory itself is untouched"
        );
        Ok(())
    }
}
