use serde::{Deserialize, Serialize};

use crate::{Arch, Flavor, Manifest};

/// One image in Waydroid's OTA format. `id` is the sha256 of the zip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaEntry {
    pub datetime: u64,
    pub filename: String,
    pub id: String,
    pub romtype: String,
    pub size: u64,
    pub url: String,
    pub version: String,
}

/// The JSON `waydroid upgrade` and the Android updater read from
/// `system_ota` and `vendor_ota`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaChannel {
    pub response: Vec<OtaEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    System,
    Vendor,
}

impl Manifest {
    /// The channel that offers Waydroid only the images this bundle pins, so it
    /// never upgrades to a build the bundle was not tested with.
    pub fn ota_channel(&self, flavor: Flavor, arch: Arch, part: Part) -> OtaChannel {
        let response = self
            .pin(flavor, arch)
            .map(|pin| match part {
                Part::System => pin.system.clone(),
                Part::Vendor => pin.vendor.clone(),
            })
            .into_iter()
            .collect();
        OtaChannel { response }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(datetime: u64) -> OtaEntry {
        OtaEntry {
            datetime,
            filename: format!("{datetime}.zip"),
            id: "0".repeat(64),
            romtype: "VANILLA".into(),
            size: 1,
            url: format!("https://example.invalid/{datetime}.zip"),
            version: "20.0".into(),
        }
    }

    #[test]
    fn the_channel_lists_only_the_pinned_image() {
        let manifest = Manifest {
            format: crate::FORMAT,
            version: "1".into(),
            waydroid: "1.6.3".into(),
            images: vec![crate::ImagePin {
                flavor: Flavor::Vanilla,
                arch: Arch::X86_64,
                system: entry(10),
                vendor: entry(20),
            }],
            files: Vec::new(),
            whiteouts: Vec::new(),
            props: Default::default(),
        };
        let channel = manifest.ota_channel(Flavor::Vanilla, Arch::X86_64, Part::Vendor);
        assert_eq!(channel.response, vec![entry(20)]);
        let other = manifest.ota_channel(Flavor::Gapps, Arch::X86_64, Part::System);
        assert!(other.response.is_empty());
    }
}
