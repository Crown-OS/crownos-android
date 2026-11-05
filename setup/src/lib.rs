//! `crownos-android-setup`: the root service that turns an installed CrownOS
//! into a ready Android runtime (contract 12), plus the bundle manifest tool
//! the CI build uses.
//!
//! Every subcommand lives in [`commands`]. The machine is reached through
//! [`Paths`] and [`Host`], so the whole flow runs against a temporary tree in
//! tests.

pub mod android_ron;
pub mod binder;
pub mod bundle_install;
pub mod cli;
pub mod commands;
pub mod failure;
pub mod host;
pub mod images;
pub mod keys;
pub mod paths;
pub mod probe;
pub mod props;
pub mod status;
pub mod util;
pub mod venus_socket;
pub mod vgem;
pub mod waydroid;

pub use failure::{FailWith, Retryable, SetupFailure};
pub use host::{Host, SystemHost};
pub use paths::Paths;
pub use status::{StatusFile, StatusSink};
