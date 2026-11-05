//! `crownos-android-setup run`: the first-boot setup service. Every stage is
//! idempotent, so a run that a reboot or a unit stop interrupted picks up
//! where it stopped. It may wait for the network for hours; nothing assumes
//! it finishes quickly.

use std::fs;
use std::io;

use anyhow::anyhow;
use bundle::{Arch, Flavor};
use crownos_config::system::AndroidConfig;
use setup_state::{FailureCode, Stage};

use super::prepare_container;
use crate::bundle_install::{self, shipped};
use crate::host::Interrupted;
use crate::images::{self, network::Backoff};
use crate::probe::{self, disk};
use crate::util::ioprio;
use crate::waydroid::init::{self, InitPlan};
use crate::{FailWith, Host, Paths, Retryable, SetupFailure, StatusSink, android_ron, keys};

/// What setup works with besides the machine itself.
pub struct Context<'a> {
    pub paths: &'a Paths,
    pub host: &'a dyn Host,
    pub public_key: &'a str,
    pub retry: Backoff,
    pub min_free_bytes: u64,
}

impl<'a> Context<'a> {
    /// The production settings.
    pub fn system(paths: &'a Paths, host: &'a dyn Host) -> Self {
        Self {
            paths,
            host,
            public_key: keys::PUBLIC_KEY,
            retry: Backoff::NETWORK,
            min_free_bytes: disk::MIN_FREE_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Android apps are off, or setup already finished.
    NothingToDo,
    Ready,
}

/// Runs setup when `android.ron` enables it and the pending marker exists.
/// A failure is recorded in `setup.json` before it is returned, except when
/// the unit was being stopped: then the last real stage stays, so the next
/// run resumes from it.
pub fn run(context: &Context, sink: &mut dyn StatusSink) -> anyhow::Result<Outcome> {
    let paths = context.paths;
    let Some(config) = android_ron::load(&paths.android_ron())?.filter(|config| config.enabled)
    else {
        log::info!("Android apps are not enabled");
        return Ok(Outcome::NothingToDo);
    };
    if !paths.pending_marker().exists() {
        log::info!("Android setup has nothing pending");
        return Ok(Outcome::NothingToDo);
    }
    if let Err(error) = ioprio::lower_to_idle() {
        log::warn!("could not lower the IO priority: {error}");
    }
    match setup(context, config, sink) {
        Ok(()) => Ok(Outcome::Ready),
        Err(failure) => {
            if failure.error.downcast_ref::<Interrupted>().is_none() {
                sink.report(failure.status())?;
            }
            Err(failure
                .error
                .context(format!("setup failed ({:?})", failure.code)))
        }
    }
}

fn setup(
    context: &Context,
    mut config: AndroidConfig,
    sink: &mut dyn StatusSink,
) -> Result<(), SetupFailure> {
    let paths = context.paths;
    report(sink, Stage::Probing, None, "Checking this computer")?;
    let probe = probe::probe(paths, context.min_free_bytes)?;
    for firewall in &probe.firewall_conflicts {
        log::warn!("{firewall} is installed; its rules can cut Android off from the network");
    }
    config.tier = Some(probe.graphics.tier());
    android_ron::save(&paths.android_ron(), &config)
        .fail_with(FailureCode::Internal, Retryable::Yes)?;

    let bundle = shipped::newest(&paths.shipped_bundles(), context.public_key)?;
    let flavor = if config.google_play {
        Flavor::Gapps
    } else {
        Flavor::Vanilla
    };
    let arch = host_arch()?;
    let pin = bundle.manifest.pin(flavor, arch).ok_or_else(|| {
        SetupFailure::new(
            FailureCode::Bundle,
            Retryable::No,
            anyhow!(
                "bundle {} pins no {flavor:?} image for {arch:?}",
                bundle.manifest.version
            ),
        )
    })?;
    let images = images::ensure_unpacked(paths, pin, context.retry, sink)?;

    report(sink, Stage::Initializing, None, "Setting up Android")?;
    let plan = InitPlan {
        manifest: &bundle.manifest,
        flavor,
        arch,
        pin,
        images: &images,
        graphics: &probe.graphics,
    };
    init::initialize(paths, context.host, &plan)?;
    images::discard_staged(paths, pin).fail_with(FailureCode::Internal, Retryable::Yes)?;

    report(
        sink,
        Stage::InstallingBundle,
        None,
        "Installing the CrownOS integration",
    )?;
    bundle_install::install(
        paths,
        context.host,
        &bundle,
        &probe.graphics,
        config.bundle.as_deref(),
    )?;
    prepare_container::sync_config_nodes(paths, probe.graphics.venus())
        .fail_with(FailureCode::Bundle, Retryable::Yes)?;
    config.bundle = Some(bundle.manifest.version.clone());
    android_ron::save(&paths.android_ron(), &config)
        .fail_with(FailureCode::Internal, Retryable::Yes)?;

    match fs::remove_file(paths.pending_marker()) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(SetupFailure::new(
                FailureCode::Internal,
                Retryable::Yes,
                error.into(),
            ));
        }
        _ => {}
    }
    report(sink, Stage::Ready, Some(100), "Android is ready")
}

fn report(
    sink: &mut dyn StatusSink,
    stage: Stage,
    percent: Option<u8>,
    message: &str,
) -> Result<(), SetupFailure> {
    sink.stage(stage, percent, message)
        .fail_with(FailureCode::Internal, Retryable::Yes)
}

fn host_arch() -> Result<Arch, SetupFailure> {
    match std::env::consts::ARCH {
        "x86_64" => Ok(Arch::X86_64),
        "aarch64" => Ok(Arch::Arm64),
        other => Err(SetupFailure::new(
            FailureCode::Bundle,
            Retryable::No,
            anyhow!("Waydroid has no images for {other}"),
        )),
    }
}
