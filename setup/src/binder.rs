//! Deterministic binder nodes on every kernel (`s3-platform.md`, root step
//! `binder-nodes`): binderfs is mounted once, `binder`, `vndbinder` and
//! `hwbinder` exist in it, and `/dev/<name>` links to them, so Waydroid finds
//! the canonical names and never allocates `anbox-*` nodes of its own.

use std::fs::{self, File, Permissions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::Path;

use anyhow::{Context, bail};
use rustix::mount::MountFlags;

use crate::Paths;
use crate::probe::kernel;
use crate::util::atomic;
use crate::waydroid::{WAYDROID_SECTION, WaydroidConfig};

/// The node names Waydroid's config and the container use.
pub const NODES: [&str; 3] = ["binder", "vndbinder", "hwbinder"];

const BINDERFS_TYPE: &str = "binder";
const NODE_MODE: u32 = 0o666;
const BINDER_CTL_ADD: libc::c_ulong = 0xC108_6201;
const BINDERFS_MAX_NAME: usize = 255;

/// `struct binderfs_device` from `linux/android/binderfs.h`.
#[repr(C)]
struct BinderfsDevice {
    name: [u8; BINDERFS_MAX_NAME + 1],
    major: u32,
    minor: u32,
}

/// Mounts binderfs if this process's mount namespace lacks it, creates the
/// three nodes and links them into `/dev`. `run` works in the setup unit's
/// private namespace, so `prepare-container` repeats this in the host's.
pub fn prepare_nodes(paths: &Paths) -> anyhow::Result<()> {
    let dev = paths.dev();
    let binderfs = dev.join("binderfs");
    let proc_root = paths.proc_root();
    if !is_binderfs_mounted(&proc_root.join("self/mountinfo"), &binderfs)? {
        if !kernel::has_binderfs(&proc_root)? {
            return require_static_nodes(&dev);
        }
        fs::create_dir_all(&binderfs)?;
        rustix::mount::mount(
            BINDERFS_TYPE,
            &binderfs,
            BINDERFS_TYPE,
            MountFlags::empty(),
            None,
        )
        .with_context(|| format!("could not mount binderfs on {}", binderfs.display()))?;
    }
    for name in NODES {
        let node = binderfs.join(name);
        if !node.exists() {
            add_device(&binderfs.join("binder-control"), name)
                .with_context(|| format!("could not create binder node {name}"))?;
        }
        fs::set_permissions(&node, Permissions::from_mode(NODE_MODE))?;
        link(&node, &dev.join(name))?;
    }
    Ok(())
}

/// Pins `waydroid.cfg` to the canonical node names, writing only on change.
pub fn pin_names(cfg_path: &Path) -> anyhow::Result<()> {
    if !cfg_path.exists() {
        return Ok(());
    }
    let mut config = WaydroidConfig::load(cfg_path)?;
    let before = config.clone();
    for name in NODES {
        config.set(WAYDROID_SECTION, name, name)?;
    }
    if config != before {
        config.save(cfg_path)?;
    }
    Ok(())
}

fn is_binderfs_mounted(mountinfo: &Path, mount_point: &Path) -> io::Result<bool> {
    let mount_point = mount_point.to_string_lossy();
    Ok(fs::read_to_string(mountinfo)?.lines().any(|line| {
        let Some((mount, filesystem)) = line.split_once(" - ") else {
            return false;
        };
        mount.split(' ').nth(4) == Some(&*mount_point)
            && filesystem.split(' ').next() == Some(BINDERFS_TYPE)
    }))
}

fn require_static_nodes(dev: &Path) -> anyhow::Result<()> {
    for name in NODES {
        let metadata = fs::metadata(dev.join(name));
        if !metadata.is_ok_and(|metadata| metadata.file_type().is_char_device()) {
            bail!("the kernel has neither binderfs nor a /dev/{name} device");
        }
    }
    Ok(())
}

fn add_device(control: &Path, name: &str) -> io::Result<()> {
    let control = File::open(control)?;
    let mut device = BinderfsDevice {
        name: [0; BINDERFS_MAX_NAME + 1],
        major: 0,
        minor: 0,
    };
    device.name[..name.len()].copy_from_slice(name.as_bytes());
    // SAFETY: BINDER_CTL_ADD reads and writes exactly one `binderfs_device`,
    // which `device` is, and the descriptor stays open for the call.
    let result = unsafe { libc::ioctl(control.as_raw_fd(), BINDER_CTL_ADD, &raw mut device) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::AlreadyExists {
        Ok(())
    } else {
        Err(error)
    }
}

/// `ln -sfn target link`, done atomically. A character device already at
/// `link` is a working static node and stays.
fn link(target: &Path, link: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(link) {
        Ok(metadata) if metadata.file_type().is_char_device() => return Ok(()),
        Ok(metadata) if metadata.is_dir() => bail!("{} is a directory", link.display()),
        Ok(metadata) if metadata.is_symlink() && fs::read_link(link)? == target => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let staged = atomic::sibling(link, "crownos-link")?;
    match fs::remove_file(&staged) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    std::os::unix::fs::symlink(target, &staged)?;
    fs::rename(&staged, link)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scratch::Scratch;

    fn mounted_tree() -> anyhow::Result<(Scratch, Paths)> {
        let root = Scratch::new("binder")?;
        let paths = Paths::under(&root);
        let binderfs = paths.dev().join("binderfs");
        fs::create_dir_all(&binderfs)?;
        for name in NODES {
            fs::write(binderfs.join(name), "")?;
        }
        fs::create_dir_all(paths.proc_root().join("self"))?;
        fs::write(paths.proc_root().join("filesystems"), "nodev\tbinder\n")?;
        fs::write(
            paths.proc_root().join("self/mountinfo"),
            format!(
                "22 1 0:21 / /proc rw,nosuid shared:13 - proc proc rw\n\
                 41 25 0:39 / {} rw,relatime shared:20 - binder binder rw,max=1048576\n",
                binderfs.display()
            ),
        )?;
        Ok((root, paths))
    }

    #[test]
    fn nodes_are_linked_into_dev_and_a_rerun_changes_nothing() -> anyhow::Result<()> {
        let (_root, paths) = mounted_tree()?;
        let dev = paths.dev();
        std::os::unix::fs::symlink("/dev/binderfs/anbox-binder", dev.join("binder"))?;
        prepare_nodes(&paths)?;
        for name in NODES {
            assert_eq!(
                fs::read_link(dev.join(name))?,
                dev.join("binderfs").join(name)
            );
            let mode = fs::metadata(dev.join("binderfs").join(name))?
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, NODE_MODE);
        }
        prepare_nodes(&paths)?;
        assert_eq!(
            fs::read_link(dev.join("binder"))?,
            dev.join("binderfs/binder")
        );
        Ok(())
    }

    #[test]
    fn mount_points_are_matched_exactly() -> anyhow::Result<()> {
        let (_root, paths) = mounted_tree()?;
        let mountinfo = paths.proc_root().join("self/mountinfo");
        assert!(is_binderfs_mounted(
            &mountinfo,
            &paths.dev().join("binderfs")
        )?);
        assert!(!is_binderfs_mounted(
            &mountinfo,
            &paths.dev().join("binder")
        )?);
        assert!(!is_binderfs_mounted(&mountinfo, Path::new("/proc"))?);
        Ok(())
    }

    #[test]
    fn a_kernel_without_any_binder_is_refused() -> anyhow::Result<()> {
        let (_root, paths) = mounted_tree()?;
        fs::write(paths.proc_root().join("self/mountinfo"), "")?;
        fs::write(paths.proc_root().join("filesystems"), "nodev\tsysfs\n")?;
        assert!(prepare_nodes(&paths).is_err());
        Ok(())
    }

    #[test]
    fn waydroid_cfg_is_pinned_to_the_canonical_names() -> anyhow::Result<()> {
        let root = Scratch::new("binder-cfg")?;
        let cfg = root.join("waydroid.cfg");
        fs::write(
            &cfg,
            "[waydroid]\nbinder = anbox-binder\nvndbinder = anbox-vndbinder\nhwbinder = anbox-hwbinder\n",
        )?;
        pin_names(&cfg)?;
        assert_eq!(
            fs::read_to_string(&cfg)?,
            "[waydroid]\nbinder = binder\nvndbinder = vndbinder\nhwbinder = hwbinder\n\n"
        );
        pin_names(&root.join("missing.cfg"))?;
        assert!(!root.join("missing.cfg").exists());
        Ok(())
    }
}
