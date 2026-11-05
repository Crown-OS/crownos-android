mod support;

use std::fs;
use std::time::Duration;

use android_setup::commands::enable;
use android_setup::commands::remove::{self, UserData};
use android_setup::commands::run::{self, Context, Outcome};
use android_setup::images::network::Backoff;
use android_setup::waydroid::WaydroidConfig;
use bundle::{Flavor, OtaChannel};
use crownos_config::system::AndroidTier;
use setup_state::{FailureCode, SetupStatus, Stage};
use support::host::FakeHost;
use support::http::Server;
use support::machine::{self, BUNDLE_VERSION, Machine, ota_entry, pin};
use support::payload;

const FAST: Backoff = Backoff {
    first: Duration::from_millis(10),
    max: Duration::from_millis(40),
};
const HWC: &str = "vendor/lib64/hw/hwcomposer.waydroid.so";

struct Prepared {
    machine: Machine,
    _server: Server,
}

/// A machine with Android enabled, a signed bundle pinning GAPPS and VANILLA
/// images, and a server holding those images.
fn prepared(label: &str) -> anyhow::Result<Prepared> {
    let machine = Machine::new(label)?;
    let gapps_zip = support::zip(&[("system.img", &payload(11, 300_000))])?;
    let vanilla_zip = support::zip(&[("system.img", &payload(12, 280_000))])?;
    let vendor_zip = support::zip(&[("vendor.img", &payload(13, 120_000))])?;
    let server = Server::start(vec![
        ("/gapps.zip", gapps_zip.clone()),
        ("/vanilla.zip", vanilla_zip.clone()),
        ("/vendor.zip", vendor_zip.clone()),
    ])?;
    let vendor = ota_entry(
        1777409843,
        "vendor.zip",
        "MAINLINE",
        server.url("/vendor.zip"),
        &vendor_zip,
    );
    let images = vec![
        pin(
            Flavor::Gapps,
            ota_entry(
                1775208984,
                "gapps.zip",
                "GAPPS",
                server.url("/gapps.zip"),
                &gapps_zip,
            ),
            vendor.clone(),
        ),
        pin(
            Flavor::Vanilla,
            ota_entry(
                1775215493,
                "vanilla.zip",
                "VANILLA",
                server.url("/vanilla.zip"),
                &vanilla_zip,
            ),
            vendor,
        ),
    ];
    let files = vec![(HWC, b"hwc".to_vec())];
    machine.ship_bundle(&machine::manifest(BUNDLE_VERSION, &files, images)?, &files)?;
    Ok(Prepared {
        machine,
        _server: server,
    })
}

fn context<'a>(machine: &'a Machine, host: &'a FakeHost, public_key: &'a str) -> Context<'a> {
    Context {
        paths: &machine.paths,
        host,
        public_key,
        retry: FAST,
        min_free_bytes: 1,
    }
}

#[test]
fn a_full_run_goes_through_every_stage_and_leaves_android_ready() -> anyhow::Result<()> {
    let prepared = prepared("run-full")?;
    let machine = &prepared.machine;
    machine.enable_android(true)?;
    let host = FakeHost::new(&machine.paths);
    let key = machine.key.public_key();
    let mut statuses: Vec<SetupStatus> = Vec::new();

    assert_eq!(
        run::run(&context(machine, &host, &key), &mut statuses)?,
        Outcome::Ready
    );

    let mut stages: Vec<Stage> = statuses.iter().map(|status| status.stage).collect();
    stages.dedup();
    assert_eq!(
        stages,
        [
            Stage::Probing,
            Stage::Downloading,
            Stage::Verifying,
            Stage::Initializing,
            Stage::InstallingBundle,
            Stage::Ready
        ]
    );
    assert_eq!(statuses.last().and_then(|status| status.percent), Some(100));

    let config = machine
        .android_config()?
        .ok_or_else(|| anyhow::anyhow!("android.ron is gone"))?;
    assert_eq!(config.tier, Some(AndroidTier::Native));
    assert_eq!(config.bundle.as_deref(), Some(BUNDLE_VERSION));
    assert!(!machine.paths.pending_marker().exists());

    let cfg = WaydroidConfig::load(&machine.paths.waydroid_cfg())?;
    let channel = machine.paths.channel();
    let system_json = channel.join("system/lineage/waydroid_x86_64/GAPPS.json");
    let vendor_json = channel.join("vendor/waydroid_x86_64/MAINLINE.json");
    let url = |path: &std::path::Path| format!("file://{}", path.display());
    assert_eq!(
        cfg.get("waydroid", "system_ota").as_deref(),
        Some(url(&system_json).as_str())
    );
    assert_eq!(
        cfg.get("waydroid", "vendor_ota").as_deref(),
        Some(url(&vendor_json).as_str())
    );
    assert_eq!(
        cfg.get("waydroid", "drm_device").as_deref(),
        Some("/dev/dri/renderD128")
    );
    assert_eq!(cfg.get("waydroid", "binder").as_deref(), Some("binder"));
    assert_eq!(
        cfg.get("waydroid", "system_datetime").as_deref(),
        Some("1775208984")
    );
    assert_eq!(
        cfg.get("properties", "ro.crownos.bundle").as_deref(),
        Some(BUNDLE_VERSION)
    );
    let images = machine.paths.images().join("1775208984-1777409843");
    assert_eq!(
        cfg.get("waydroid", "images_path").as_deref(),
        Some(images.to_string_lossy().as_ref())
    );

    let system: OtaChannel = serde_json::from_slice(&fs::read(&system_json)?)?;
    assert_eq!(system.response.len(), 1);
    assert_eq!(system.response[0].romtype, "GAPPS");
    assert_eq!(
        system.response[0].url,
        url(&machine.paths.staging().join("gapps.zip"))
    );
    let vendor: OtaChannel = serde_json::from_slice(&fs::read(&vendor_json)?)?;
    assert_eq!(
        vendor.response[0].url,
        url(&machine.paths.staging().join("vendor.zip"))
    );
    assert!(!channel.join("system.json").exists() && !channel.join("vendor.json").exists());
    assert!(
        host.fetched.borrow().is_empty(),
        "the seeded build times keep init from copying the zips"
    );
    assert_eq!(
        fs::read_dir(machine.paths.staging())?.count(),
        0,
        "the zips go once init has run"
    );
    assert!(
        host.calls()
            .iter()
            .any(|call| call.ends_with("-r lineage -s GAPPS"))
    );
    let nodes = fs::read_to_string(machine.paths.config_nodes())?;
    assert!(nodes.contains("lxc.mount.entry = /dev/binder dev/binder"));
    assert_eq!(fs::read(machine.paths.overlay().join(HWC))?, b"hwc");
    assert_eq!(
        fs::read_link(machine.paths.dev().join("hwbinder"))?,
        machine.paths.dev().join("binderfs/hwbinder")
    );
    assert!(
        host.calls()
            .iter()
            .any(|call| call.starts_with("waydroid init -f -i "))
    );
    assert_eq!(
        host.calls().last().map(String::as_str),
        Some("waydroid upgrade --offline")
    );

    let mut rerun: Vec<SetupStatus> = Vec::new();
    assert_eq!(
        run::run(&context(machine, &host, &key), &mut rerun)?,
        Outcome::NothingToDo
    );
    assert!(rerun.is_empty());
    Ok(())
}

#[test]
fn a_run_interrupted_after_the_download_resumes_without_downloading_again() -> anyhow::Result<()> {
    let prepared = prepared("run-resume")?;
    let machine = &prepared.machine;
    machine.enable_android(false)?;
    let host = FakeHost::new(&machine.paths);
    let key = machine.key.public_key();
    let mut statuses: Vec<SetupStatus> = Vec::new();
    run::run(&context(machine, &host, &key), &mut statuses)?;
    fs::write(machine.paths.pending_marker(), "")?;
    let requests_before = prepared._server.requests().len();

    let mut resumed: Vec<SetupStatus> = Vec::new();
    assert_eq!(
        run::run(&context(machine, &host, &key), &mut resumed)?,
        Outcome::Ready
    );
    assert_eq!(prepared._server.requests().len(), requests_before);
    assert!(
        !resumed
            .iter()
            .any(|status| status.stage == Stage::Downloading)
    );
    let cfg = WaydroidConfig::load(&machine.paths.waydroid_cfg())?;
    assert_eq!(
        cfg.get("waydroid", "system_datetime").as_deref(),
        Some("1775215493")
    );
    Ok(())
}

#[test]
fn a_stopped_unit_leaves_the_last_real_stage_for_the_next_run() -> anyhow::Result<()> {
    let prepared = prepared("run-stopped")?;
    let machine = &prepared.machine;
    machine.enable_android(true)?;
    let host = FakeHost::new(&machine.paths);
    host.interrupt_init.set(true);
    let key = machine.key.public_key();
    let mut statuses: Vec<SetupStatus> = Vec::new();

    assert!(run::run(&context(machine, &host, &key), &mut statuses).is_err());
    assert_eq!(
        statuses.last().map(|status| status.stage),
        Some(Stage::Initializing)
    );
    assert!(
        machine.paths.staging().join("gapps.zip").exists(),
        "init still needs the staged zips"
    );

    let mut resumed: Vec<SetupStatus> = Vec::new();
    assert_eq!(
        run::run(&context(machine, &host, &key), &mut resumed)?,
        Outcome::Ready
    );
    assert!(
        !resumed
            .iter()
            .any(|status| status.stage == Stage::Downloading)
    );
    Ok(())
}

#[test]
fn an_unsupported_kernel_fails_for_good_and_says_why() -> anyhow::Result<()> {
    let prepared = prepared("run-page-size")?;
    let machine = &prepared.machine;
    machine.enable_android(true)?;
    machine.set_page_size(16384)?;
    let host = FakeHost::new(&machine.paths);
    let key = machine.key.public_key();
    let mut statuses: Vec<SetupStatus> = Vec::new();

    assert!(run::run(&context(machine, &host, &key), &mut statuses).is_err());
    let failure = statuses.last().and_then(|status| status.failure.clone());
    assert_eq!(
        failure.as_ref().map(|failure| failure.code),
        Some(FailureCode::UnsupportedPageSize)
    );
    assert_eq!(failure.map(|failure| failure.retryable), Some(false));
    assert!(machine.paths.pending_marker().exists());
    assert!(host.calls().is_empty());
    Ok(())
}

#[test]
fn a_bundle_signed_by_another_key_stops_setup() -> anyhow::Result<()> {
    let prepared = prepared("run-signature")?;
    let machine = &prepared.machine;
    machine.enable_android(true)?;
    let host = FakeHost::new(&machine.paths);
    let other_key = support::sign::TestKey::new(42).public_key();
    let mut statuses: Vec<SetupStatus> = Vec::new();
    assert!(run::run(&context(machine, &host, &other_key), &mut statuses).is_err());
    let code = statuses
        .last()
        .and_then(|status| status.failure.as_ref())
        .map(|failure| failure.code);
    assert_eq!(code, Some(FailureCode::Signature));
    let config = machine
        .android_config()?
        .ok_or_else(|| anyhow::anyhow!("android.ron is gone"))?;
    assert_eq!(
        config.tier,
        Some(AndroidTier::Native),
        "the probe result is kept"
    );
    Ok(())
}

#[test]
fn nothing_happens_when_android_is_off() -> anyhow::Result<()> {
    let prepared = prepared("run-off")?;
    let machine = &prepared.machine;
    let host = FakeHost::new(&machine.paths);
    let key = machine.key.public_key();
    let mut statuses: Vec<SetupStatus> = Vec::new();
    assert_eq!(
        run::run(&context(machine, &host, &key), &mut statuses)?,
        Outcome::NothingToDo
    );
    machine.enable_android(true)?;
    fs::remove_file(machine.paths.pending_marker())?;
    assert_eq!(
        run::run(&context(machine, &host, &key), &mut statuses)?,
        Outcome::NothingToDo
    );
    assert!(statuses.is_empty());
    Ok(())
}

#[test]
fn remove_undoes_setup_and_wipes_user_data_on_request() -> anyhow::Result<()> {
    let prepared = prepared("remove")?;
    let machine = &prepared.machine;
    machine.enable_android(true)?;
    let host = FakeHost::new(&machine.paths);
    let key = machine.key.public_key();
    run::run(
        &context(machine, &host, &key),
        &mut Vec::<SetupStatus>::new(),
    )?;
    fs::write(machine.paths.pending_marker(), "")?;
    let home = machine.root.join("home/alice");
    fs::create_dir_all(home.join(".local/share/waydroid/data"))?;
    fs::create_dir_all(home.join(".local/share/kept"))?;
    let elsewhere = machine.root.join("srv/precious");
    fs::create_dir_all(elsewhere.join("share/waydroid"))?;
    let mallory = machine.root.join("home/mallory");
    fs::create_dir_all(&mallory)?;
    std::os::unix::fs::symlink(&elsewhere, mallory.join(".local"))?;
    fs::write(
        machine.paths.passwd(),
        "root:x:0:0::/root:/bin/bash\nalice:x:1000:1000::/home/alice:/bin/fish\nmallory:x:1001:1001::/home/mallory:/bin/sh\n",
    )?;

    remove::remove(&machine.paths, &host, UserData::Wipe)?;

    for gone in [
        machine.paths.images(),
        machine.paths.channel(),
        machine.paths.pending_marker(),
    ] {
        assert!(!gone.exists(), "{}", gone.display());
    }
    assert!(!machine.paths.overlay().join(HWC).exists());
    assert!(machine.paths.overlay().join("vendor").is_dir());
    let config = machine
        .android_config()?
        .ok_or_else(|| anyhow::anyhow!("android.ron is gone"))?;
    assert!(!config.enabled);
    assert!(!home.join(".local/share/waydroid").exists());
    assert!(home.join(".local/share/kept").exists());
    assert!(
        elsewhere.join("share/waydroid").exists(),
        "a symlinked ~/.local is never followed"
    );
    let calls = host.calls();
    assert!(calls.contains(&"systemctl stop waydroid-container.service".to_owned()));
    assert!(calls.contains(&"systemctl disable crownos-android-setup.service".to_owned()));
    Ok(())
}

#[test]
fn enable_turns_setup_back_on_and_keeps_the_choices() -> anyhow::Result<()> {
    let prepared = prepared("enable")?;
    let machine = &prepared.machine;
    machine.enable_android(true)?;
    let host = FakeHost::new(&machine.paths);
    let key = machine.key.public_key();
    run::run(
        &context(machine, &host, &key),
        &mut Vec::<SetupStatus>::new(),
    )?;
    remove::remove(&machine.paths, &host, UserData::Keep)?;

    for _ in 0..2 {
        enable::enable(&machine.paths, &host)?;
        let config = machine
            .android_config()?
            .ok_or_else(|| anyhow::anyhow!("android.ron is gone"))?;
        assert!(config.enabled && config.google_play);
        assert_eq!(config.tier, Some(AndroidTier::Native));
        assert!(machine.paths.pending_marker().exists());
        let status = setup_state::load(&machine.paths.status())?;
        assert_eq!(status.map(|status| status.stage), Some(Stage::Queued));
    }
    let starts = host
        .calls()
        .into_iter()
        .filter(|call| call == "systemctl start --no-block crownos-android-setup.service")
        .count();
    assert_eq!(starts, 2);

    assert_eq!(
        run::run(
            &context(machine, &host, &key),
            &mut Vec::<SetupStatus>::new()
        )?,
        Outcome::Ready
    );
    Ok(())
}

#[test]
fn enable_creates_a_missing_android_ron() -> anyhow::Result<()> {
    let machine = Machine::new("enable-fresh")?;
    let host = FakeHost::new(&machine.paths);
    enable::enable(&machine.paths, &host)?;
    let config = machine
        .android_config()?
        .ok_or_else(|| anyhow::anyhow!("android.ron was not created"))?;
    assert!(config.enabled && !config.google_play);
    assert!(machine.paths.pending_marker().exists());
    Ok(())
}
