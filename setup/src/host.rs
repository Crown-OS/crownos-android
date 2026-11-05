use std::ffi::OsStr;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

use anyhow::{Context, bail};

const WAYDROID: &str = "/usr/bin/waydroid";
const SYSTEMCTL: &str = "/usr/bin/systemctl";

/// The programs setup drives. Tests swap in a fake that records the calls.
pub trait Host {
    fn waydroid(&self, args: &[&OsStr]) -> anyhow::Result<()>;
    fn systemctl(&self, args: &[&str]) -> anyhow::Result<()>;
}

/// A child was killed by a signal, as when systemd stops the unit. It says
/// nothing about the machine, so setup records no failure for it.
#[derive(Debug, thiserror::Error)]
#[error("{program} was stopped by signal {signal}")]
pub struct Interrupted {
    pub program: String,
    pub signal: i32,
}

/// The real programs; their output goes to the journal with ours.
pub struct SystemHost;

impl Host for SystemHost {
    fn waydroid(&self, args: &[&OsStr]) -> anyhow::Result<()> {
        run(Command::new(WAYDROID).args(args))
    }

    fn systemctl(&self, args: &[&str]) -> anyhow::Result<()> {
        run(Command::new(SYSTEMCTL).args(args))
    }
}

fn run(command: &mut Command) -> anyhow::Result<()> {
    let program = command.get_program().display().to_string();
    let status = command
        .status()
        .with_context(|| format!("could not start {program}"))?;
    if let Some(signal) = status.signal() {
        return Err(Interrupted { program, signal }.into());
    }
    if !status.success() {
        bail!("{command:?} failed: {status}");
    }
    Ok(())
}
