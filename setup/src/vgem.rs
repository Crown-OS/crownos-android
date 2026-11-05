//! vgem, the virtual GEM driver whose fences the CrownOS hwcomposer hands
//! SurfaceFlinger in place of sw_sync, which the CrownOS kernels lack
//! (`docs/decisions/s3-platform.md`). Waydroid binds only the GPU's render
//! node into the container, so `prepare-container` adds vgem's.

use std::path::{Path, PathBuf};

const DRIVER: &str = "vgem";

/// Mount options of the line CrownOS adds. `nosuid` sets it apart from the
/// render node line Waydroid writes for the GPU, so a stale line is told from
/// Waydroid's own; `optional` keeps the container starting without vgem.
const MOUNT_OPTIONS: &str = "bind,create=file,optional,nosuid";

/// vgem's render node, such as `/dev/dri/renderD130`, when the module is loaded.
pub fn render_node(sys: &Path) -> Option<PathBuf> {
    let drm = sys.join("class/drm");
    std::fs::read_dir(&drm)
        .ok()?
        .flatten()
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy().starts_with("renderD"))
        .find(|name| is_vgem(&drm.join(name)))
        .map(|name| Path::new("/dev/dri").join(name))
}

/// The LXC entry binding `node` into the container at the same path.
pub fn mount_line(node: &Path) -> String {
    let node = node.display().to_string();
    let inside = node.trim_start_matches('/');
    format!("lxc.mount.entry = {node} {inside} none {MOUNT_OPTIONS} 0 0")
}

/// Whether `line` is an entry [`mount_line`] wrote, for any node.
pub fn is_mount_line(line: &str) -> bool {
    line.split_whitespace().nth(5) == Some(MOUNT_OPTIONS)
}

/// The device is named vgem; older kernels also name its driver vgem.
fn is_vgem(drm_entry: &Path) -> bool {
    ["device", "device/driver"].iter().any(|link| {
        std::fs::read_link(drm_entry.join(link))
            .is_ok_and(|target| target.file_name().is_some_and(|name| name == DRIVER))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn fake_sys(name: &str) -> std::io::Result<PathBuf> {
        let sys = std::env::temp_dir().join(format!("vgem-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&sys);
        std::fs::create_dir_all(sys.join("class/drm"))?;
        std::fs::create_dir_all(sys.join("devices/pci0000:00/0000:06:00.0"))?;
        std::fs::create_dir_all(sys.join("devices/faux/vgem"))?;
        Ok(sys)
    }

    fn drm_entry(sys: &Path, entry: &str, device: &str) -> std::io::Result<()> {
        let dir = sys.join("class/drm").join(entry);
        std::fs::create_dir_all(&dir)?;
        symlink(sys.join(device), dir.join("device"))
    }

    #[test]
    fn finds_the_vgem_render_node_and_skips_the_gpu() -> std::io::Result<()> {
        let sys = fake_sys("found")?;
        drm_entry(&sys, "renderD128", "devices/pci0000:00/0000:06:00.0")?;
        drm_entry(&sys, "card1", "devices/faux/vgem")?;
        drm_entry(&sys, "renderD130", "devices/faux/vgem")?;
        assert_eq!(
            render_node(&sys),
            Some(PathBuf::from("/dev/dri/renderD130"))
        );
        std::fs::remove_dir_all(&sys)
    }

    #[test]
    fn no_vgem_means_no_node() -> std::io::Result<()> {
        let sys = fake_sys("absent")?;
        drm_entry(&sys, "renderD128", "devices/pci0000:00/0000:06:00.0")?;
        assert_eq!(render_node(&sys), None);
        std::fs::remove_dir_all(&sys)
    }

    #[test]
    fn its_mount_line_is_recognised_and_waydroids_is_not() {
        let line = mount_line(Path::new("/dev/dri/renderD130"));
        assert_eq!(
            line,
            "lxc.mount.entry = /dev/dri/renderD130 dev/dri/renderD130 none bind,create=file,optional,nosuid 0 0"
        );
        assert!(is_mount_line(&line));
        assert!(!is_mount_line(
            "lxc.mount.entry = /dev/dri/renderD128 dev/dri/renderD128 none bind,create=file,optional 0 0"
        ));
    }
}
