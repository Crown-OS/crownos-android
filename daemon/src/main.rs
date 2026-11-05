//! `crown-androidd`: starts Android on demand, shows the splash, stops
//! Android when it sits idle, keeps the menu in step with the installed apps
//! and turns setup progress into a notification.

mod actions;
mod activation;
mod daemon;
mod dbus_host;
mod deferred;
mod diagnose;
mod effects;
mod event_loop;
mod handler;
mod host_worker;
mod notice;
mod notify;
mod settings;
mod setup;
#[cfg(test)]
mod tests;
mod util;
mod wayland;

use std::path::PathBuf;
use std::sync::Arc;

use activation::Launch;
use android_protocol::android;
use crownos_ipc::ServiceBuilder;
use runtime::Paths;
use runtime::host::Buses;

use crate::daemon::{Daemon, Environment, Services};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let builder = ServiceBuilder::new(android::SERVICE);
    let (server, launch) = match activation::inherited_listener() {
        Some(listener) => (builder.build_with_listener(listener)?, Launch::Systemd),
        None => (builder.build()?, Launch::Manual),
    };
    let (notifier, inbox) = notice::notice_channel()?;
    let _idle_minutes = settings::follow_idle_minutes(notifier.clone());
    let buses = Arc::new(Buses::default());
    let services = Services {
        host: Box::new(dbus_host::DbusHost::new(Arc::clone(&buses))),
        buses: Some(buses),
        wayland: true,
        tone: settings::tone(),
    };
    let env = Environment {
        paths: Paths::from_env()?,
        system_root: PathBuf::from("/"),
    };
    let daemon = Daemon::new(env, services, notifier, settings::idle_minutes())?;
    event_loop::run(server, daemon, inbox, launch)
}
