mod support;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use bundle::Manifest;
use support::Scratch;

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../overlay/build-bundle.sh");
const VERSION: &str = "2026.11.0";
/// Writes the `.minisig` that `minisign -S ... -x <file>` would, without a key.
const FAKE_MINISIGN: &str = "#!/bin/sh
while [ \"$#\" -gt 0 ]; do
    [ \"$1\" = -x ] && printf 'untrusted comment: test signature\\n' >\"$2\"
    shift
done
";

fn artifacts(root: &Path) -> std::io::Result<()> {
    let hw = root.join("hwc/vendor/lib64/hw");
    fs::create_dir_all(&hw)?;
    fs::write(hw.join("hwcomposer.waydroid.so"), "hwc")?;
    let agent = root.join("agent");
    fs::create_dir_all(&agent)?;
    for (name, bytes) in [
        ("crown-agent.jar", "jar"),
        ("crown-agent.rc", "rc"),
        ("CrownHome.apk", "home"),
        ("CrownSystemUIOverlay.apk", "systemui"),
        ("CrownFrameworkOverlay.apk", "framework"),
    ] {
        fs::write(agent.join(name), bytes)?;
    }
    Ok(())
}

fn build(root: &Path) -> Command {
    let mut command = Command::new("bash");
    command
        .arg(SCRIPT)
        .args(["--version", VERSION, "--out"])
        .arg(root.join("out"))
        .arg("--setup")
        .arg(env!("CARGO_BIN_EXE_crownos-android-setup"))
        .env("HWC_OUT", root.join("hwc"))
        .env("AGENT_OUT", root.join("agent"));
    command
}

/// A `PATH` whose `minisign` is [`FAKE_MINISIGN`].
fn path_with_fake_minisign(root: &Path) -> anyhow::Result<OsString> {
    let bin = root.join("bin");
    fs::create_dir_all(&bin)?;
    let minisign = bin.join("minisign");
    fs::write(&minisign, FAKE_MINISIGN)?;
    fs::set_permissions(&minisign, fs::Permissions::from_mode(0o755))?;
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = vec![bin];
    dirs.extend(std::env::split_paths(&inherited));
    Ok(std::env::join_paths(dirs)?)
}

fn tar(args: &[&OsStr]) -> anyhow::Result<String> {
    let output = Command::new("tar")
        .args(["-I", "zstd"])
        .args(args)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn the_script_places_every_artifact_and_writes_the_manifest() -> anyhow::Result<()> {
    let root = Scratch::new("build-bundle")?;
    artifacts(&root)?;
    let output = build(&root).output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tree = root.join("out").join(VERSION);
    let manifest: Manifest = serde_json::from_slice(&fs::read(tree.join("manifest.json"))?)?;
    let paths: Vec<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "system/etc/init/crown-agent.rc",
            "system/framework/crown-agent.jar",
            "system/product/app/CrownHome/CrownHome.apk",
            "system/product/overlay/CrownFrameworkOverlay.apk",
            "system/product/overlay/CrownSystemUIOverlay.apk",
            "vendor/lib64/hw/hwcomposer.waydroid.so",
        ]
    );
    assert!(manifest.files.iter().all(|file| file.mode == 0o644));
    manifest.check_files(&tree)?;
    assert!(!tree.join("manifest.json.minisig").exists());
    assert!(
        !root.join(format!("out/bundle-{VERSION}.tar.zst")).exists(),
        "unsigned bundles are not packed"
    );
    Ok(())
}

#[test]
fn a_missing_artifact_names_the_build_that_makes_it() -> anyhow::Result<()> {
    let root = Scratch::new("build-bundle-missing")?;
    artifacts(&root)?;
    fs::remove_file(root.join("hwc/vendor/lib64/hw/hwcomposer.waydroid.so"))?;
    let output = build(&root).output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hwcomposer.waydroid.so") && stderr.contains("ndk/build.sh"),
        "{stderr}"
    );
    assert!(!root.join("out").join(VERSION).exists());
    Ok(())
}

#[test]
fn a_signed_bundle_unpacks_with_its_manifest_at_the_archive_root() -> anyhow::Result<()> {
    let root = Scratch::new("build-bundle-signed")?;
    artifacts(&root)?;
    let key = root.join("dev.key");
    fs::write(&key, "secret")?;
    let output = build(&root)
        .arg("--key")
        .arg(&key)
        .env("PATH", path_with_fake_minisign(&root)?)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let archive = root.join(format!("out/bundle-{VERSION}.tar.zst"));
    let listing = tar(&["-tf".as_ref(), archive.as_os_str()])?;
    let entries: Vec<&str> = listing.lines().collect();
    for top_level in [
        "./manifest.json",
        "./manifest.json.minisig",
        "./system/",
        "./vendor/",
    ] {
        assert!(entries.contains(&top_level), "{top_level} in {entries:?}");
    }
    assert!(
        entries.iter().all(|entry| !entry.contains(VERSION)),
        "no entry sits below a version directory: {entries:?}"
    );

    let unpacked = root
        .join("package/usr/share/crownos-android/bundle")
        .join(VERSION);
    fs::create_dir_all(&unpacked)?;
    tar(&[
        "-xf".as_ref(),
        archive.as_os_str(),
        "-C".as_ref(),
        unpacked.as_os_str(),
    ])?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(unpacked.join("manifest.json"))?)?;
    assert_eq!(manifest.version, VERSION);
    manifest.check_files(&unpacked)?;
    assert!(unpacked.join("manifest.json.minisig").is_file());
    Ok(())
}
