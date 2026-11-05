//! The splash window an Android app shows from the moment it is launched
//! until its real window maps, and the daemon's Wayland connection that
//! carries it.
//!
//! The window is an `xdg_toplevel` with the app's `app_id` and name, marked as
//! a placeholder through `crownos-android-shell-v1` so the compositor hands
//! its place to the app's window. Its background is a 1×1 buffer the
//! compositor scales; only the icon is drawn, with tiny-skia, at the
//! window's preferred fractional scale.
//!
//! The pure parts, [`geometry`], [`color`] and [`layout`], carry the rules;
//! [`Splashes`] does the Wayland work.

mod client;
pub mod color;
pub mod geometry;
mod icon;
pub mod layout;
mod shm;

pub use client::{SecurityContext, SplashError, SplashRequest, Splashes};
pub use color::{Rgb, Tone};
pub use geometry::{LogicalSize, PhysicalSize, Scale, launch_size};
