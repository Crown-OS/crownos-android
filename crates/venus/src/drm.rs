use std::path::{Path, PathBuf};

use crate::VenusError;
use crate::util::kernel_files;

const DRM_CLASS: &str = "class/drm";
const CARD_PREFIX: &str = "card";
const RENDER_NODE_PREFIX: &str = "renderD";
const DEVICE_NODE_DIR: &str = "/dev/dri";
const BOOT_VGA: &str = "1";
const DRIVER_KEY: &str = "DRIVER=";

/// A DRM card, seen through its sysfs device directory.
pub struct Card {
    device: PathBuf,
}

impl Card {
    /// The card the compositor renders on, by the rule of smithay's
    /// `udev::primary_gpu` that crownpositor uses: the boot VGA device,
    /// otherwise the first card, by name, that has a render node.
    pub fn compositor(sys: &Path) -> Result<Option<Self>, VenusError> {
        let class = sys.join(DRM_CLASS);
        let mut names = kernel_files::entry_names(&class)?;
        names.retain(|name| is_card_name(name));
        names.sort_unstable();

        let mut first_with_render_node = None;
        for name in names {
            let card = Self {
                device: class.join(name).join("device"),
            };
            if card.is_boot_vga()? {
                return Ok(Some(card));
            }
            if first_with_render_node.is_none() && card.render_node()?.is_some() {
                first_with_render_node = Some(card);
            }
        }
        Ok(first_with_render_node)
    }

    pub fn is_driven_by(&self, driver: &str) -> Result<bool, VenusError> {
        let uevent = kernel_files::read_value(&self.device.join("uevent"))?;
        Ok(uevent.is_some_and(|uevent| {
            uevent
                .lines()
                .any(|line| line.strip_prefix(DRIVER_KEY) == Some(driver))
        }))
    }

    /// The card's render node as a device path, for example `/dev/dri/renderD128`.
    pub fn render_node(&self) -> Result<Option<PathBuf>, VenusError> {
        let nodes = kernel_files::entry_names(&self.device.join("drm"))?;
        Ok(nodes
            .into_iter()
            .find(|node| node.starts_with(RENDER_NODE_PREFIX))
            .map(|node| Path::new(DEVICE_NODE_DIR).join(node)))
    }

    fn is_boot_vga(&self) -> Result<bool, VenusError> {
        let boot_vga = kernel_files::read_value(&self.device.join("boot_vga"))?;
        Ok(boot_vga.as_deref() == Some(BOOT_VGA))
    }
}

/// `card0` is a card; `card0-eDP-1` is one of its connectors.
fn is_card_name(name: &str) -> bool {
    name.strip_prefix(CARD_PREFIX)
        .is_some_and(|index| !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connectors_and_render_nodes_are_not_cards() {
        assert!(is_card_name("card0"));
        assert!(is_card_name("card12"));
        assert!(!is_card_name("card0-eDP-1"));
        assert!(!is_card_name("renderD128"));
        assert!(!is_card_name("card"));
        assert!(!is_card_name("version"));
    }
}
