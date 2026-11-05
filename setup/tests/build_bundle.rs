mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use bundle::Manifest;
use support::Scratch;

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../overlay/build-bundle.sh");

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

fn build(root: &Path) -> std::io::Result<Output> {
    Command::new("bash")
        .arg(SCRIPT)
        .args(["--version", "2026.11.0", "--out"])
        .arg(root.join("out"))
        .arg("--setup")
        .arg(env!("CARGO_BIN_EXE_crownos-android-setup"))
        .env("HWC_OUT", root.join("hwc"))
        .env("AGENT_OUT", root.join("agent"))
        .output()
}

#[test]
fn the_script_places_every_artifact_and_writes_the_manifest() -> anyhow::Result<()> {
    let root = Scratch::new("build-bundle")?;
    artifacts(&root)?;
    let output = build(&root)?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tree = root.join("out/2026.11.0");
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
        !root.join("out/bundle-2026.11.0.tar.zst").exists(),
        "unsigned bundles are not packed"
    );
    Ok(())
}

#[test]
fn a_missing_artifact_names_the_build_that_makes_it() -> anyhow::Result<()> {
    let root = Scratch::new("build-bundle-missing")?;
    artifacts(&root)?;
    fs::remove_file(root.join("hwc/vendor/lib64/hw/hwcomposer.waydroid.so"))?;
    let output = build(&root)?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hwcomposer.waydroid.so") && stderr.contains("ndk/build.sh"),
        "{stderr}"
    );
    assert!(!root.join("out/2026.11.0").exists());
    Ok(())
}
