use std::ffi::OsStr;
use std::path::Path;

use bundle::{Arch, Flavor, ImagePin, Manifest};
use setup_state::FailureCode;

use super::cfg::CfgError;
use super::channel::{self, Channel};
use super::{WAYDROID_SECTION, WaydroidConfig};
use crate::probe::Graphics;
use crate::{FailWith, Host, Paths, Retryable, SetupFailure, binder};

/// What `waydroid init` is pointed at.
pub struct InitPlan<'a> {
    pub manifest: &'a Manifest,
    pub flavor: Flavor,
    pub arch: Arch,
    pub pin: &'a ImagePin,
    pub images: &'a Path,
    pub graphics: &'a Graphics,
}

/// Runs `waydroid init -f -i <images>` on the unpacked pinned images
/// without a download: `waydroid.cfg` is seeded beforehand with the channel
/// URLs init computes and the pinned build times, so Waydroid finds nothing
/// newer, and the channel lists only the staged zips. Afterwards the binder
/// names and `drm_device` are pinned.
pub fn initialize(paths: &Paths, host: &dyn Host, plan: &InitPlan) -> Result<(), SetupFailure> {
    binder::prepare_nodes(paths).fail_with(FailureCode::NoBinder, Retryable::No)?;
    let channel = Channel::new(paths.channel(), plan.flavor, plan.arch);
    channel
        .write(paths, plan.manifest, plan.flavor, plan.arch)
        .fail_with(FailureCode::Init, Retryable::Yes)?;

    let cfg_path = paths.waydroid_cfg();
    let pin_channel = |config: &mut WaydroidConfig| -> Result<(), CfgError> {
        config.set(
            WAYDROID_SECTION,
            "system_ota",
            &channel::file_url(&channel.system_json()),
        )?;
        config.set(
            WAYDROID_SECTION,
            "vendor_ota",
            &channel::file_url(&channel.vendor_json()),
        )?;
        set_drm_device(config, plan.graphics)
    };
    WaydroidConfig::edit(&cfg_path, |config| {
        pin_channel(config)?;
        config.set(
            WAYDROID_SECTION,
            "system_datetime",
            &plan.pin.system.datetime.to_string(),
        )?;
        config.set(
            WAYDROID_SECTION,
            "vendor_datetime",
            &plan.pin.vendor.datetime.to_string(),
        )
    })
    .fail_with(FailureCode::Init, Retryable::Yes)?;

    let system_channel = channel::file_url(&channel.system_channel());
    let vendor_channel = channel::file_url(&channel.vendor_channel());
    let args = [
        OsStr::new("init"),
        OsStr::new("-f"),
        OsStr::new("-i"),
        plan.images.as_os_str(),
        OsStr::new("-c"),
        OsStr::new(&system_channel),
        OsStr::new("-v"),
        OsStr::new(&vendor_channel),
        OsStr::new("-r"),
        OsStr::new(channel::ROM_TYPE),
        OsStr::new("-s"),
        OsStr::new(channel.system_type()),
    ];
    host.waydroid(&args)
        .fail_with(FailureCode::Init, Retryable::Yes)?;

    WaydroidConfig::edit(&cfg_path, |config| {
        pin_channel(config)?;
        for name in binder::NODES {
            config.set(WAYDROID_SECTION, name, name)?;
        }
        Ok(())
    })
    .fail_with(FailureCode::Init, Retryable::Yes)
}

fn set_drm_device(config: &mut WaydroidConfig, graphics: &Graphics) -> Result<(), CfgError> {
    match graphics.drm_device() {
        Some(node) => config.set(WAYDROID_SECTION, "drm_device", &node.to_string_lossy())?,
        None => config.remove(WAYDROID_SECTION, "drm_device"),
    }
    Ok(())
}
