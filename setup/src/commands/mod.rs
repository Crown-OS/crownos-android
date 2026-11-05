//! The subcommands of contract 12.

pub mod enable;
pub mod make_manifest;
pub mod prepare_container;
pub mod remove;
pub mod run;

/// The first-boot unit that runs `crownos-android-setup run`.
pub const SETUP_UNIT: &str = "crownos-android-setup.service";
/// Waydroid's system unit, which owns the container.
pub const CONTAINER_UNIT: &str = "waydroid-container.service";
