mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use android_setup::commands::make_manifest::{Pins, make_manifest};
use bundle::{Flavor, Manifest};
use support::machine::{ota_entry, pin};
use support::sign::TestKey;
use support::{Scratch, sha256_hex};

fn tree(label: &str) -> anyhow::Result<Scratch> {
    let root = Scratch::new(label)?;
    let files = [
        ("vendor/lib64/hw/hwcomposer.waydroid.so", &b"hwc"[..], 0o644),
        ("system/framework/crown-agent.jar", b"jar", 0o644),
        ("system/etc/init/crown-agent.rc", b"rc", 0o644),
        ("system/product/app/CrownHome/CrownHome.apk", b"home", 0o644),
        ("system/bin/crown-tool", b"tool", 0o755),
    ];
    for (path, bytes, mode) in files {
        let target = root.join(path);
        fs::create_dir_all(
            target
                .parent()
                .ok_or_else(|| anyhow::anyhow!("no parent"))?,
        )?;
        fs::write(&target, bytes)?;
        fs::set_permissions(&target, fs::Permissions::from_mode(mode))?;
    }
    fs::write(
        root.join("whiteouts.txt"),
        "# stale SystemUI oat\nsystem/system_ext/priv-app/SystemUI/oat\n\n",
    )?;
    let pins = Pins {
        waydroid: "1.6.3".into(),
        images: vec![pin(
            Flavor::Gapps,
            ota_entry(
                1775208984,
                "system.zip",
                "GAPPS",
                "https://example.invalid/s.zip".into(),
                b"s",
            ),
            ota_entry(
                1777409843,
                "vendor.zip",
                "MAINLINE",
                "https://example.invalid/v.zip".into(),
                b"v",
            ),
        )],
    };
    fs::write(root.join("pins.json"), serde_json::to_vec_pretty(&pins)?)?;
    Ok(root)
}

#[test]
fn the_manifest_lists_every_file_and_parses_as_a_signed_manifest() -> anyhow::Result<()> {
    let root = tree("manifest")?;
    let written = make_manifest(&root, "2026.11.0", &root.join("pins.json"))?;
    assert_eq!(written, root.join("manifest.json"));
    let bytes = fs::read(&written)?;

    let key = TestKey::new(3);
    let manifest = Manifest::verify(&bytes, &key.sign(&bytes), &key.public_key())?;
    assert_eq!(manifest.version, "2026.11.0");
    assert_eq!(manifest.waydroid, "1.6.3");
    let paths: Vec<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "system/bin/crown-tool",
            "system/etc/init/crown-agent.rc",
            "system/framework/crown-agent.jar",
            "system/product/app/CrownHome/CrownHome.apk",
            "vendor/lib64/hw/hwcomposer.waydroid.so",
        ]
    );
    assert_eq!(manifest.files[0].mode, 0o755);
    assert_eq!(manifest.files[4].sha256, sha256_hex(b"hwc"));
    assert_eq!(manifest.whiteouts.len(), 1);
    assert_eq!(
        manifest.props.get("ro.crownos.bundle").map(String::as_str),
        Some("2026.11.0")
    );
    assert_eq!(
        manifest
            .pin(Flavor::Gapps, bundle::Arch::X86_64)
            .map(|pin| pin.vendor.datetime),
        Some(1777409843)
    );
    manifest.check_files(&root)?;
    Ok(())
}

#[test]
fn symlinks_bad_whiteouts_and_clashes_are_refused() -> anyhow::Result<()> {
    let root = tree("manifest-refused")?;
    std::os::unix::fs::symlink("/etc/passwd", root.join("system/etc/passwd"))?;
    assert!(make_manifest(&root, "2026.11.0", &root.join("pins.json")).is_err());
    fs::remove_file(root.join("system/etc/passwd"))?;

    fs::write(root.join("whiteouts.txt"), "../outside\n")?;
    assert!(make_manifest(&root, "2026.11.0", &root.join("pins.json")).is_err());

    fs::write(root.join("whiteouts.txt"), "system/bin/crown-tool\n")?;
    assert!(make_manifest(&root, "2026.11.0", &root.join("pins.json")).is_err());

    fs::remove_file(root.join("whiteouts.txt"))?;
    assert!(make_manifest(&root, "../2026", &root.join("pins.json")).is_err());
    assert!(make_manifest(&root, "2026.11.0", &root.join("pins.json")).is_ok());
    Ok(())
}

#[test]
fn the_committed_pins_parse() -> anyhow::Result<()> {
    let pins: Pins = serde_json::from_slice(&fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../overlay/pins.json"
    ))?)?;
    assert!(
        pins.images
            .iter()
            .any(|pin| pin.flavor == Flavor::Gapps && pin.system.datetime == 1775208984)
    );
    assert!(pins.images.iter().any(|pin| pin.flavor == Flavor::Vanilla));
    assert!(
        pins.images
            .iter()
            .all(|pin| pin.vendor.datetime == 1777409843)
    );
    Ok(())
}
