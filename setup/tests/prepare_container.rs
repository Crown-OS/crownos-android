mod support;

use std::fs;

use android_setup::commands::prepare_container::prepare_container;
use android_setup::waydroid::config_nodes;
use crownos_config::system::{AndroidConfig, AndroidTier};
use support::machine::Machine;

#[test]
fn binder_nodes_and_their_config_are_pinned_idempotently() -> anyhow::Result<()> {
    let machine = Machine::new("prepare")?;
    fs::write(
        machine.paths.waydroid_cfg(),
        "[waydroid]\nbinder = anbox-binder\nvndbinder = anbox-vndbinder\nhwbinder = anbox-hwbinder\n",
    )?;
    let nodes = machine.paths.config_nodes();
    fs::create_dir_all(nodes.parent().ok_or_else(|| anyhow::anyhow!("no parent"))?)?;
    fs::write(
        &nodes,
        "lxc.mount.entry = /dev/anbox-binder dev/binder none bind,create=file,optional 0 0\n\
         lxc.mount.entry = /dev/uhid dev/uhid none bind,create=file,optional 0 0\n",
    )?;
    let config = AndroidConfig {
        enabled: true,
        google_play: false,
        tier: Some(AndroidTier::Native),
        bundle: Some("2026.11.0".into()),
    };
    android_setup::android_ron::save(&machine.paths.android_ron(), &config)?;

    prepare_container(&machine.paths)?;
    let written = fs::read_to_string(&nodes)?;
    assert!(written.starts_with(&config_nodes::binder_line("binder")));
    assert!(written.contains("dev/uhid"));
    assert!(written.contains(&config_nodes::binder_line("hwbinder")));
    let cfg = fs::read_to_string(machine.paths.waydroid_cfg())?;
    assert!(
        cfg.contains("binder = binder\nvndbinder = vndbinder\nhwbinder = hwbinder"),
        "{cfg}"
    );
    for name in ["binder", "vndbinder", "hwbinder"] {
        assert_eq!(
            fs::read_link(machine.paths.dev().join(name))?,
            machine.paths.dev().join("binderfs").join(name)
        );
    }

    let modified = fs::metadata(&nodes)?.modified()?;
    prepare_container(&machine.paths)?;
    assert_eq!(
        fs::metadata(&nodes)?.modified()?,
        modified,
        "an unchanged file is not rewritten"
    );
    assert_eq!(fs::read_to_string(&nodes)?, written);
    Ok(())
}

#[test]
fn an_uninitialised_waydroid_is_left_for_init() -> anyhow::Result<()> {
    let machine = Machine::new("prepare-fresh")?;
    prepare_container(&machine.paths)?;
    assert!(!machine.paths.config_nodes().exists());
    assert!(!machine.paths.waydroid_cfg().exists());
    Ok(())
}
