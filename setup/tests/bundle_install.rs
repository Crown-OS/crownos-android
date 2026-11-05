mod support;

use std::fs;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::PathBuf;

use android_setup::bundle_install::{self, shipped};
use android_setup::probe::Graphics;
use bundle::OverlayPath;
use setup_state::FailureCode;
use support::host::FakeHost;
use support::machine::{self, BUNDLE_VERSION, Machine};

const HWC: &str = "vendor/lib64/hw/hwcomposer.waydroid.so";
const AGENT: &str = "system/framework/crown-agent.jar";
const RC: &str = "system/etc/init/crown-agent.rc";

fn bundle_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (HWC, b"hwc".to_vec()),
        (AGENT, b"agent".to_vec()),
        (
            RC,
            b"service crown-agent /system/bin/app_process\n".to_vec(),
        ),
    ]
}

/// A `waydroid.cfg` as `waydroid init` leaves it, which `upgrade` needs.
fn initialized(label: &str, properties: &str) -> anyhow::Result<Machine> {
    let machine = Machine::new(label)?;
    fs::write(
        machine.paths.waydroid_cfg(),
        format!(
            "[waydroid]\nmount_overlays = False\nbinder = binder\nvndbinder = vndbinder\nhwbinder = hwbinder\n\n[properties]\n{properties}"
        ),
    )?;
    Ok(machine)
}

fn ship(
    machine: &Machine,
    version: &str,
    files: &[(&str, Vec<u8>)],
) -> anyhow::Result<shipped::ShippedBundle> {
    let manifest = machine::manifest(version, files, Vec::new())?;
    machine.ship_bundle(&manifest, files)?;
    Ok(shipped::newest(
        &machine.paths.shipped_bundles(),
        &machine.key.public_key(),
    )?)
}

fn snapshot(root: &std::path::Path) -> anyhow::Result<Vec<(PathBuf, Vec<u8>, u32)>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                let mode = entry.metadata()?.permissions().mode();
                found.push((
                    entry.path(),
                    fs::read(entry.path()).unwrap_or_default(),
                    mode,
                ));
            }
        }
    }
    found.sort();
    Ok(found)
}

#[test]
fn the_bundle_lands_in_the_overlay_with_its_props_and_installs_idempotently() -> anyhow::Result<()>
{
    let machine = initialized("install", "ro.user.kept = yes\n")?;
    fs::create_dir_all(machine.paths.overlay_rw().join("system/stale"))?;
    let bundle = ship(&machine, BUNDLE_VERSION, &bundle_files())?;
    let host = FakeHost::new(&machine.paths);
    let graphics = Graphics::Native("/dev/dri/renderD128".into());

    bundle_install::install(&machine.paths, &host, &bundle, &graphics, None)
        .map_err(|failure| failure.error)?;

    let overlay = machine.paths.overlay();
    assert_eq!(fs::read(overlay.join(HWC))?, b"hwc");
    assert_eq!(
        fs::metadata(overlay.join(AGENT))?.permissions().mode() & 0o777,
        0o644
    );
    assert!(!machine.paths.overlay_rw().exists());
    let cfg = fs::read_to_string(machine.paths.waydroid_cfg())?;
    assert!(cfg.contains("mount_overlays = True"), "{cfg}");
    assert!(cfg.contains("ro.user.kept = yes"));
    assert!(cfg.contains(&format!("ro.crownos.bundle = {BUNDLE_VERSION}")));
    assert!(cfg.contains("persist.waydroid.width = 4096"));
    assert!(!cfg.contains("ro.hardware.egl"));
    assert_eq!(host.calls(), ["waydroid upgrade --offline"]);
    let record = machine.paths.installed_bundles().join(BUNDLE_VERSION);
    assert_eq!(
        fs::read(record.join(shipped::MANIFEST))?,
        bundle.manifest_bytes
    );

    let first = (snapshot(&overlay)?, cfg);
    bundle_install::install(
        &machine.paths,
        &host,
        &bundle,
        &graphics,
        Some(BUNDLE_VERSION),
    )
    .map_err(|failure| failure.error)?;
    assert_eq!(
        (
            snapshot(&overlay)?,
            fs::read_to_string(machine.paths.waydroid_cfg())?
        ),
        first
    );
    Ok(())
}

#[test]
fn a_new_bundle_removes_the_files_the_old_one_no_longer_lists() -> anyhow::Result<()> {
    let machine = initialized("install-upgrade", "")?;
    let host = FakeHost::new(&machine.paths);
    let old = ship(&machine, "2026.10.0", &bundle_files())?;
    bundle_install::install(&machine.paths, &host, &old, &Graphics::SwiftShader, None)
        .map_err(|failure| failure.error)?;
    assert!(machine.paths.overlay().join(RC).exists());
    fs::remove_dir_all(old.dir)?;

    let new = ship(&machine, BUNDLE_VERSION, &bundle_files()[..1])?;
    bundle_install::install(
        &machine.paths,
        &host,
        &new,
        &Graphics::SwiftShader,
        Some("2026.10.0"),
    )
    .map_err(|failure| failure.error)?;
    let overlay = machine.paths.overlay();
    assert!(overlay.join(HWC).exists());
    assert!(!overlay.join(AGENT).exists() && !overlay.join("system/framework").exists());
    assert!(!overlay.join("system/etc").exists());
    assert!(
        overlay.join("system").is_dir(),
        "the overlay roots stay for Waydroid's mounts"
    );
    let cfg = fs::read_to_string(machine.paths.waydroid_cfg())?;
    assert!(cfg.contains("ro.hardware.egl = swiftshader"));
    Ok(())
}

#[test]
fn the_venus_tier_adds_its_overlay_files_and_props() -> anyhow::Result<()> {
    let machine = initialized("install-venus", "")?;
    for path in venus::overlay_files() {
        let source = machine.paths.venus_overlay().join(path);
        fs::create_dir_all(
            source
                .parent()
                .ok_or_else(|| anyhow::anyhow!("no parent"))?,
        )?;
        fs::write(source, path.as_bytes())?;
    }
    let bundle = ship(&machine, BUNDLE_VERSION, &bundle_files())?;
    let graphics = Graphics::Venus(venus::Support {
        render_node: "/dev/dri/renderD129".into(),
        driver_version: "615.78.08".into(),
    });
    bundle_install::install(
        &machine.paths,
        &FakeHost::new(&machine.paths),
        &bundle,
        &graphics,
        None,
    )
    .map_err(|failure| failure.error)?;
    for path in venus::overlay_files() {
        assert_eq!(
            fs::read(machine.paths.overlay().join(path))?,
            path.as_bytes()
        );
    }
    let cfg = fs::read_to_string(machine.paths.waydroid_cfg())?;
    assert!(
        cfg.contains("gralloc.gbm.device = /dev/dri/renderD129"),
        "{cfg}"
    );
    assert!(cfg.contains("mesa.vtest.socket.name = /data/crownos/venus/venus.sock"));
    Ok(())
}

#[test]
fn whiteouts_become_character_devices() -> anyhow::Result<()> {
    let machine = initialized("install-whiteout", "")?;
    let probe = machine.root.join("mknod-probe");
    if rustix::fs::mknodat(
        rustix::fs::CWD,
        &probe,
        rustix::fs::FileType::CharacterDevice,
        rustix::fs::Mode::empty(),
        rustix::fs::makedev(0, 0),
    )
    .is_err()
    {
        eprintln!("skipped: this user cannot create whiteout devices");
        return Ok(());
    }
    let whiteout = "system/system_ext/priv-app/SystemUI/oat";
    fs::create_dir_all(machine.paths.overlay().join(whiteout).join("x86_64"))?;
    let mut manifest = machine::manifest(BUNDLE_VERSION, &bundle_files(), Vec::new())?;
    manifest
        .whiteouts
        .push(OverlayPath::try_from(whiteout.to_owned())?);
    machine.ship_bundle(&manifest, &bundle_files())?;
    let bundle = shipped::newest(&machine.paths.shipped_bundles(), &machine.key.public_key())?;
    let host = FakeHost::new(&machine.paths);
    for _ in 0..2 {
        bundle_install::install(&machine.paths, &host, &bundle, &Graphics::SwiftShader, None)
            .map_err(|failure| failure.error)?;
        let metadata = fs::symlink_metadata(machine.paths.overlay().join(whiteout))?;
        assert!(metadata.file_type().is_char_device());
    }
    Ok(())
}

#[test]
fn a_file_that_does_not_match_its_hash_is_not_installed() -> anyhow::Result<()> {
    let machine = initialized("install-tampered", "")?;
    let bundle = ship(&machine, BUNDLE_VERSION, &bundle_files())?;
    fs::write(bundle.dir.join(HWC), b"tampered")?;
    let failure = bundle_install::install(
        &machine.paths,
        &FakeHost::new(&machine.paths),
        &bundle,
        &Graphics::SwiftShader,
        None,
    );
    assert!(
        matches!(&failure, Err(failure) if failure.code == FailureCode::Bundle),
        "{failure:?}"
    );
    assert!(!machine.paths.overlay().join(HWC).exists());
    Ok(())
}

#[test]
fn manifests_with_escaping_paths_or_bad_signatures_are_rejected() -> anyhow::Result<()> {
    let machine = Machine::new("install-rejected")?;
    let manifest = machine::manifest(BUNDLE_VERSION, &bundle_files(), Vec::new())?;
    let dir = machine.ship_bundle(&manifest, &bundle_files())?;

    let hostile = fs::read_to_string(dir.join(shipped::MANIFEST))?.replace(HWC, "../../etc/shadow");
    fs::write(dir.join(shipped::MANIFEST), &hostile)?;
    fs::write(
        dir.join(shipped::SIGNATURE),
        machine.key.sign(hostile.as_bytes()),
    )?;
    let escaping = shipped::load(&dir, &machine.key.public_key());
    assert!(
        matches!(&escaping, Err(failure) if failure.code == FailureCode::Bundle),
        "{escaping:?}"
    );

    let other_key = support::sign::TestKey::new(9);
    let unsigned = shipped::load(&dir, &other_key.public_key());
    assert!(
        matches!(&unsigned, Err(failure) if failure.code == FailureCode::Signature),
        "{unsigned:?}"
    );

    fs::remove_file(dir.join(shipped::SIGNATURE))?;
    let missing = shipped::load(&dir, &machine.key.public_key());
    assert!(
        matches!(&missing, Err(failure) if failure.code == FailureCode::Signature),
        "{missing:?}"
    );
    Ok(())
}

#[test]
fn the_newest_shipped_version_wins_and_must_match_its_directory() -> anyhow::Result<()> {
    let machine = Machine::new("install-newest")?;
    for version in ["2026.9.0", "2026.11.0", "2026.10.2"] {
        let manifest = machine::manifest(version, &bundle_files(), Vec::new())?;
        machine.ship_bundle(&manifest, &bundle_files())?;
    }
    let newest = shipped::newest(&machine.paths.shipped_bundles(), &machine.key.public_key())?;
    assert_eq!(newest.manifest.version, "2026.11.0");

    let misnamed = machine.paths.shipped_bundles().join("2027.1.0");
    fs::rename(&newest.dir, &misnamed)?;
    let refused = shipped::newest(&machine.paths.shipped_bundles(), &machine.key.public_key());
    assert!(
        matches!(&refused, Err(failure) if failure.code == FailureCode::Bundle),
        "{refused:?}"
    );
    Ok(())
}
