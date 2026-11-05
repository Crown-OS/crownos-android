use std::io;
use std::path::{Path, PathBuf};

use crownos_config::system::AndroidTier;
use venus::{Support, VenusError};

/// Drivers the vendor image's Mesa renders on natively.
const NATIVE_DRIVERS: [&str; 8] = [
    "amdgpu", "i915", "xe", "msm", "panfrost", "panthor", "asahi", "nouveau",
];

/// Drivers Mesa cannot render on: NVIDIA's own, and vgem, a fence-only device
/// that Waydroid's `gpu.py` would otherwise pick (`s3-platform.md`).
const NOT_FOR_MESA: [&str; 2] = ["nvidia", "vgem"];

const DEV_DRI: &str = "/dev/dri";

const SWIFTSHADER_PROPS: [(&str, &str); 2] = [
    ("ro.hardware.gralloc", "default"),
    ("ro.hardware.egl", "swiftshader"),
];

/// One DRM card under `/sys/class/drm`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrmCard {
    pub name: String,
    pub driver: String,
    pub boot_vga: bool,
    pub render_node: Option<String>,
}

/// How Android renders on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Graphics {
    /// Mesa on the compositor's GPU, at this render node.
    Native(PathBuf),
    /// Venus to the host render server on the compositor's NVIDIA GPU.
    Venus(Support),
    /// Mesa on a render node the compositor does not drive.
    OtherGpu(PathBuf),
    /// SwiftShader on the CPU.
    SwiftShader,
}

impl Graphics {
    pub fn tier(&self) -> AndroidTier {
        match self {
            Self::Native(_) => AndroidTier::Native,
            Self::Venus(_) => AndroidTier::Venus,
            Self::OtherGpu(_) | Self::SwiftShader => AndroidTier::Software,
        }
    }

    /// The render node Waydroid's `drm_device` must name.
    pub fn drm_device(&self) -> Option<&Path> {
        match self {
            Self::Native(node) | Self::OtherGpu(node) => Some(node),
            Self::Venus(support) => Some(&support.render_node),
            Self::SwiftShader => None,
        }
    }

    pub fn venus(&self) -> Option<&Support> {
        match self {
            Self::Venus(support) => Some(support),
            _ => None,
        }
    }

    /// Properties this tier adds to `waydroid.cfg [properties]`.
    pub fn props(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Venus(support) => venus::props(support),
            Self::SwiftShader => SWIFTSHADER_PROPS
                .iter()
                .map(|(key, value)| (*key, (*value).to_owned()))
                .collect(),
            Self::Native(_) | Self::OtherGpu(_) => Vec::new(),
        }
    }
}

/// Every `cardN` under `sys/class/drm`, sorted by name.
pub fn cards(sys: &Path) -> io::Result<Vec<DrmCard>> {
    let class = sys.join("class/drm");
    let entries = match std::fs::read_dir(&class) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut cards = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| is_card(name))
        .map(|name| read_card(&class, name))
        .collect::<io::Result<Vec<_>>>()?;
    cards.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(cards)
}

fn is_card(name: &str) -> bool {
    name.strip_prefix("card").is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn read_card(class: &Path, name: String) -> io::Result<DrmCard> {
    let device = class.join(&name).join("device");
    let driver = read_optional(&device.join("uevent"))?
        .lines()
        .find_map(|line| line.strip_prefix("DRIVER="))
        .unwrap_or_default()
        .to_owned();
    let boot_vga = read_optional(&device.join("boot_vga"))?.trim() == "1";
    let render_node = match std::fs::read_dir(device.join("drm")) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|node| node.starts_with("renderD"))
            .min(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    Ok(DrmCard {
        name,
        driver,
        boot_vga,
        render_node,
    })
}

fn read_optional(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        other => other,
    }
}

/// The GPU crownpositor composites on, by Smithay's `udev::primary_gpu` rule:
/// the boot_vga card, else the first card with a render node, else the first card.
pub fn compositor_gpu(cards: &[DrmCard]) -> Option<&DrmCard> {
    cards
        .iter()
        .find(|card| card.boot_vga)
        .or_else(|| cards.iter().find(|card| card.render_node.is_some()))
        .or_else(|| cards.first())
}

/// Picks the tier: Native on a Mesa driver, Venus when `detect_venus` finds
/// support, otherwise Software on any other render node Mesa can use, else
/// SwiftShader.
pub fn choose(
    cards: &[DrmCard],
    detect_venus: impl FnOnce() -> Result<Option<Support>, VenusError>,
) -> Result<Graphics, VenusError> {
    if let Some(card) = compositor_gpu(cards)
        && NATIVE_DRIVERS.contains(&card.driver.as_str())
        && let Some(node) = &card.render_node
    {
        return Ok(Graphics::Native(dev_node(node)));
    }
    if let Some(support) = detect_venus()? {
        return Ok(Graphics::Venus(support));
    }
    Ok(cards
        .iter()
        .filter(|card| !NOT_FOR_MESA.contains(&card.driver.as_str()))
        .find_map(|card| card.render_node.as_deref())
        .map_or(Graphics::SwiftShader, |node| {
            Graphics::OtherGpu(dev_node(node))
        }))
}

/// [`choose`] over the machine's sysfs and procfs.
pub fn detect(sys: &Path, proc_root: &Path) -> anyhow::Result<Graphics> {
    Ok(choose(&cards(sys)?, || venus::detect(sys, proc_root))?)
}

fn dev_node(name: &str) -> PathBuf {
    Path::new(DEV_DRI).join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scratch::Scratch;

    struct Card<'a> {
        name: &'a str,
        driver: &'a str,
        boot_vga: Option<bool>,
        render: Option<&'a str>,
    }

    fn sysfs(cards: &[Card]) -> io::Result<Scratch> {
        let sys = Scratch::new("gpu")?;
        let class = sys.join("class/drm");
        std::fs::create_dir_all(class.join("version"))?;
        for card in cards {
            let device = class.join(card.name).join("device");
            std::fs::create_dir_all(device.join("drm").join(card.name))?;
            std::fs::create_dir_all(class.join(format!("{}-eDP-1", card.name)))?;
            std::fs::write(
                device.join("uevent"),
                format!("DRIVER={}\nPCI_ID=1002:15BF\n", card.driver),
            )?;
            if let Some(boot_vga) = card.boot_vga {
                std::fs::write(
                    device.join("boot_vga"),
                    if boot_vga { "1\n" } else { "0\n" },
                )?;
            }
            if let Some(render) = card.render {
                std::fs::create_dir_all(device.join("drm").join(render))?;
            }
        }
        Ok(sys)
    }

    fn no_venus() -> Result<Option<Support>, VenusError> {
        Ok(None)
    }

    fn venus_on(node: &str) -> impl FnOnce() -> Result<Option<Support>, VenusError> {
        let support = Support {
            render_node: PathBuf::from(node),
            driver_version: "615.78.08".into(),
        };
        move || Ok(Some(support))
    }

    fn never_asked() -> Result<Option<Support>, VenusError> {
        Err(VenusError::Read {
            path: "venus must not be probed".into(),
            source: io::ErrorKind::Unsupported.into(),
        })
    }

    fn graphics(
        cards: &[Card],
        detect_venus: impl FnOnce() -> Result<Option<Support>, VenusError>,
    ) -> Graphics {
        let sys = sysfs(cards).ok();
        let found = sys
            .as_deref()
            .map(super::cards)
            .and_then(Result::ok)
            .unwrap_or_default();
        choose(&found, detect_venus).unwrap_or(Graphics::SwiftShader)
    }

    const AMD: Card = Card {
        name: "card1",
        driver: "amdgpu",
        boot_vga: Some(true),
        render: Some("renderD128"),
    };
    const NVIDIA_DGPU: Card = Card {
        name: "card0",
        driver: "nvidia",
        boot_vga: Some(false),
        render: Some("renderD129"),
    };
    const NVIDIA_PRIMARY: Card = Card {
        name: "card0",
        driver: "nvidia",
        boot_vga: Some(true),
        render: Some("renderD129"),
    };
    const AMD_SECONDARY: Card = Card {
        name: "card1",
        driver: "amdgpu",
        boot_vga: Some(false),
        render: Some("renderD128"),
    };
    const VGEM: Card = Card {
        name: "card2",
        driver: "vgem",
        boot_vga: None,
        render: Some("renderD130"),
    };

    #[test]
    fn a_mesa_gpu_runs_the_compositor_natively() {
        assert_eq!(
            graphics(&[AMD], never_asked),
            Graphics::Native("/dev/dri/renderD128".into())
        );
        let intel = Card {
            name: "card0",
            driver: "i915",
            ..AMD
        };
        assert_eq!(graphics(&[intel], never_asked).tier(), AndroidTier::Native);
    }

    #[test]
    fn a_hybrid_laptop_stays_on_the_boot_gpu() {
        assert_eq!(
            graphics(&[NVIDIA_DGPU, AMD], never_asked),
            Graphics::Native("/dev/dri/renderD128".into())
        );
    }

    #[test]
    fn a_platform_gpu_without_boot_vga_is_found_by_its_render_node() {
        let msm = Card {
            name: "card0",
            driver: "msm",
            boot_vga: None,
            render: Some("renderD128"),
        };
        let display_only = Card {
            name: "card1",
            driver: "simpledrm",
            boot_vga: None,
            render: None,
        };
        assert_eq!(
            graphics(&[msm, display_only], never_asked).tier(),
            AndroidTier::Native
        );
    }

    #[test]
    fn an_nvidia_compositor_gpu_uses_venus_when_supported() {
        let chosen = graphics(&[NVIDIA_PRIMARY, VGEM], venus_on("/dev/dri/renderD129"));
        assert_eq!(chosen.tier(), AndroidTier::Venus);
        assert_eq!(chosen.drm_device(), Some(Path::new("/dev/dri/renderD129")));
        assert!(
            chosen
                .props()
                .iter()
                .any(|(key, _)| *key == "mesa.vtest.socket.name")
        );
    }

    #[test]
    fn without_venus_software_rendering_uses_another_mesa_gpu() {
        let chosen = graphics(&[NVIDIA_PRIMARY, AMD_SECONDARY], no_venus);
        assert_eq!(chosen, Graphics::OtherGpu("/dev/dri/renderD128".into()));
        assert_eq!(chosen.tier(), AndroidTier::Software);
        assert!(chosen.props().is_empty());
    }

    #[test]
    fn an_nvidia_only_machine_without_venus_falls_back_to_swiftshader() {
        let chosen = graphics(&[NVIDIA_PRIMARY, VGEM], no_venus);
        assert_eq!(chosen, Graphics::SwiftShader);
        assert_eq!(chosen.drm_device(), None);
        assert!(
            chosen
                .props()
                .contains(&("ro.hardware.egl", "swiftshader".into()))
        );
    }

    #[test]
    fn a_virtual_gpu_renders_in_software_mode_on_its_node() {
        let virtio = Card {
            name: "card0",
            driver: "virtio_gpu",
            boot_vga: Some(true),
            render: Some("renderD128"),
        };
        assert_eq!(
            graphics(&[virtio], no_venus),
            Graphics::OtherGpu("/dev/dri/renderD128".into())
        );
    }

    #[test]
    fn a_machine_without_drm_uses_swiftshader() {
        assert_eq!(graphics(&[], no_venus), Graphics::SwiftShader);
        assert_eq!(choose(&[], no_venus).ok(), Some(Graphics::SwiftShader));
    }

    #[test]
    fn connectors_and_the_version_file_are_not_cards() -> io::Result<()> {
        let sys = sysfs(&[AMD, NVIDIA_DGPU])?;
        let names: Vec<_> = cards(&sys)?.into_iter().map(|card| card.name).collect();
        assert_eq!(names, ["card0", "card1"]);
        Ok(())
    }
}
