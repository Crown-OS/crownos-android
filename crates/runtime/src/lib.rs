//! Starts, stops, freezes and watches the Waydroid runtime for one user.
//!
//! [`lifecycle::Lifecycle`] is the pure state machine: it takes events and a
//! clock reading and answers with [`lifecycle::Effect`]s. Everything that
//! touches the machine lives in thin adapters: [`agent`] speaks agent-wire with
//! crown-agent, [`host`] drives systemd, Waydroid and logind over D-Bus, and
//! [`watch`] follows files with inotify.

pub mod agent;
pub mod host;
pub mod installation;
pub mod lifecycle;
pub mod paths;
pub mod persisted;
pub mod watch;

mod control;
mod util;

pub use control::supplement;
pub use installation::{Installation, Installed};
pub use lifecycle::{Effect, Errand, Lifecycle, Milestone};
pub use paths::Paths;
pub use persisted::Persisted;
