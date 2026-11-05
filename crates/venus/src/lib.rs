//! The Venus GPU tier: Android on an NVIDIA-only machine renders through a
//! patched venus driver over a vtest socket to a host render server, which
//! drives NVIDIA's own Vulkan driver (`docs/decisions/s4-venus.md`).
//!
//! The setup service calls [`detect`], [`props`], [`overlay_files`] and
//! [`container_mount_line`]; `crown-androidd` starts [`RENDER_SERVICE`] before
//! the session. These signatures are contract 13; lane L10 owns the bodies.

use std::path::{Path, PathBuf};

/// User unit running the host render server.
pub const RENDER_SERVICE: &str = "crownos-android-render.service";

/// The vtest socket as Android sees it.
pub const SOCKET_GUEST: &str = "/data/crownos/venus/venus.sock";

/// Where the `crownos-android-venus` package keeps the overlay files it adds.
pub const OVERLAY_SOURCE: &str = "/usr/share/crownos-android/venus/overlay";

/// Lowest NVIDIA driver major version with the Vulkan extensions the chain needs.
pub const MIN_DRIVER_MAJOR: u32 = 595;

/// An NVIDIA GPU that can run the Venus tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Support {
    /// Render node of the GPU the compositor runs on, for example `/dev/dri/renderD128`.
    pub render_node: PathBuf,
    pub driver_version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum VenusError {
    #[error("could not read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
}

/// The Venus support of this machine, reading sysfs under `sys` and procfs
/// under `proc_root`. `None` means the machine uses another tier: the
/// compositor GPU is not NVIDIA, the kernel module is closed, the driver is
/// older than [`MIN_DRIVER_MAJOR`], modeset is off or `/dev/udmabuf` is missing.
pub fn detect(sys: &Path, proc_root: &Path) -> Result<Option<Support>, VenusError> {
    let _ = (sys, proc_root);
    Ok(None)
}

/// Overlay paths, relative to the overlay root, that [`OVERLAY_SOURCE`] provides.
pub fn overlay_files() -> &'static [&'static str] {
    &[
        "vendor/lib64/hw/vulkan.virtio.so",
        "vendor/lib/hw/vulkan.virtio.so",
        "vendor/lib64/libgbm_mesa_wrapper.so",
        "vendor/lib/libgbm_mesa_wrapper.so",
    ]
}

/// The Android properties of the Venus tier.
pub fn props(support: &Support) -> Vec<(&'static str, String)> {
    vec![
        ("ro.hardware.gralloc", "minigbm_gbm_mesa".into()),
        (
            "gralloc.gbm.device",
            support.render_node.display().to_string(),
        ),
        ("ro.hardware.egl", "angle".into()),
        ("ro.hardware.vulkan", "virtio".into()),
        ("mesa.vn.debug", "vtest".into()),
        ("mesa.vtest.socket.name", SOCKET_GUEST.into()),
        ("debug.hwui.renderer", "skiavk".into()),
        ("debug.renderengine.backend", "skiagl".into()),
        ("ro.opengles.version", "196610".into()),
    ]
}

/// The LXC mount entry that gives the container the NVIDIA render node, which
/// stock Waydroid leaves out.
pub fn container_mount_line(support: &Support) -> String {
    let node = support.render_node.display().to_string();
    let inside = node.trim_start_matches('/');
    format!("lxc.mount.entry = {node} {inside} none bind,create=file 0 0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn support() -> Support {
        Support {
            render_node: PathBuf::from("/dev/dri/renderD129"),
            driver_version: "615.78.08".into(),
        }
    }

    #[test]
    fn the_mount_line_binds_the_render_node_into_the_container() {
        assert_eq!(
            container_mount_line(&support()),
            "lxc.mount.entry = /dev/dri/renderD129 dev/dri/renderD129 none bind,create=file 0 0"
        );
    }

    #[test]
    fn gralloc_allocates_on_the_nvidia_node() {
        let props = props(&support());
        assert!(props.contains(&("gralloc.gbm.device", "/dev/dri/renderD129".into())));
    }
}
