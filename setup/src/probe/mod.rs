//! What this machine offers Android, read from sysfs and procfs only, so every
//! decision is a pure function of a directory tree.

pub mod disk;
pub mod firewall;
pub mod gpu;
pub mod kernel;

pub use gpu::Graphics;

use anyhow::anyhow;
use setup_state::FailureCode;

use crate::{FailWith, Paths, Retryable, SetupFailure};

/// The probe's findings.
#[derive(Debug)]
pub struct Probe {
    pub graphics: Graphics,
    /// Firewalls whose NAT rules can block Android's network (the iso lane ships the fix).
    pub firewall_conflicts: Vec<&'static str>,
}

/// Checks binder, the page size and free disk space, then picks the GPU tier.
pub fn probe(paths: &Paths, min_free_bytes: u64) -> Result<Probe, SetupFailure> {
    let proc_root = paths.proc_root();
    if !kernel::has_binderfs(&proc_root).fail_with(FailureCode::Internal, Retryable::Yes)? {
        return Err(unsupported(
            FailureCode::NoBinder,
            "binderfs is not in /proc/filesystems",
        ));
    }
    let page_size =
        kernel::page_size(&proc_root).fail_with(FailureCode::Internal, Retryable::Yes)?;
    if !kernel::page_size_supported(page_size) {
        return Err(unsupported(
            FailureCode::UnsupportedPageSize,
            format!("the kernel page size is {page_size:?} bytes"),
        ));
    }
    let free =
        disk::free_bytes(&paths.disk_checked()).fail_with(FailureCode::Internal, Retryable::Yes)?;
    if free < min_free_bytes {
        return Err(unsupported(
            FailureCode::NotEnoughDisk,
            format!("{free} bytes are free on /var/lib, {min_free_bytes} are needed"),
        ));
    }
    let graphics =
        gpu::detect(&paths.sys(), &proc_root).fail_with(FailureCode::Internal, Retryable::Yes)?;
    Ok(Probe {
        graphics,
        firewall_conflicts: firewall::conflicts(paths.root()),
    })
}

fn unsupported(code: FailureCode, reason: impl Into<String>) -> SetupFailure {
    SetupFailure::new(code, Retryable::No, anyhow!(reason.into()))
}
