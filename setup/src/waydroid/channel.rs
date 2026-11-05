use std::path::{Path, PathBuf};

use anyhow::Context;
use bundle::{Arch, Flavor, Manifest, OtaChannel, Part};

use crate::Paths;
use crate::images;
use crate::util::atomic;

/// The rom type `waydroid init` puts into the system channel path.
pub const ROM_TYPE: &str = "lineage";
/// The vendor type `waydroid init` asks for on a PC (`get_vendor_type`).
pub const VENDOR_TYPE: &str = "MAINLINE";

/// The pinned OTA channel under `/var/lib/crownos/android/channel/`, in
/// ota.waydro.id's layout, which `channels.cfg` names:
/// `system/lineage/waydroid_<arch>/<GAPPS|VANILLA>.json` and
/// `vendor/waydroid_<arch>/MAINLINE.json`.
pub struct Channel {
    root: PathBuf,
    arch: &'static str,
    system_type: &'static str,
}

impl Channel {
    pub fn new(root: PathBuf, flavor: Flavor, arch: Arch) -> Self {
        Self {
            root,
            arch: waydroid_arch(arch),
            system_type: system_type(flavor),
        }
    }

    /// `waydroid init -c`.
    pub fn system_channel(&self) -> PathBuf {
        self.root.join("system")
    }

    /// `waydroid init -v`.
    pub fn vendor_channel(&self) -> PathBuf {
        self.root.join("vendor")
    }

    /// `waydroid init -s`.
    pub fn system_type(&self) -> &'static str {
        self.system_type
    }

    /// The `system_ota` Waydroid derives from the channel arguments.
    pub fn system_json(&self) -> PathBuf {
        self.system_channel()
            .join(ROM_TYPE)
            .join(format!("waydroid_{}", self.arch))
            .join(format!("{}.json", self.system_type))
    }

    /// The `vendor_ota` Waydroid derives for a mainline host.
    pub fn vendor_json(&self) -> PathBuf {
        self.vendor_channel()
            .join(format!("waydroid_{}", self.arch))
            .join(format!("{VENDOR_TYPE}.json"))
    }

    /// Writes both channel files from [`Manifest::ota_channel`], so Waydroid
    /// only ever sees the pinned images. Each entry's URL is the staged,
    /// verified zip, so even a `waydroid init` that decides to fetch an image
    /// never touches the network.
    pub fn write(
        &self,
        paths: &Paths,
        manifest: &Manifest,
        flavor: Flavor,
        arch: Arch,
    ) -> anyhow::Result<()> {
        for (path, part) in [
            (self.system_json(), Part::System),
            (self.vendor_json(), Part::Vendor),
        ] {
            let mut channel = manifest.ota_channel(flavor, arch, part);
            point_at_staging(paths, &mut channel);
            let json = serde_json::to_vec_pretty(&channel)?;
            atomic::write(&path, &json)
                .with_context(|| format!("could not write {}", path.display()))?;
        }
        Ok(())
    }
}

fn point_at_staging(paths: &Paths, channel: &mut OtaChannel) {
    for entry in &mut channel.response {
        entry.url = file_url(&images::staged_zip(paths, entry));
    }
}

/// A `file://` URL for an absolute path, as Python's urllib opens it.
pub fn file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Waydroid's name for the architecture (`helpers/arch.py`).
pub const fn waydroid_arch(arch: Arch) -> &'static str {
    match arch {
        Arch::X86_64 => "x86_64",
        Arch::Arm64 => "arm64",
    }
}

/// Waydroid's system type for the flavor the user chose.
pub const fn system_type(flavor: Flavor) -> &'static str {
    match flavor {
        Flavor::Gapps => "GAPPS",
        Flavor::Vanilla => "VANILLA",
    }
}
