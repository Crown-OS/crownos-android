use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use android_setup::waydroid::WaydroidConfig;
use android_setup::{Host, Paths};
use anyhow::{Context, bail, ensure};

const BINDER_NAMES: [[&str; 4]; 3] = [
    ["anbox-binder", "puddlejumper", "bonder", "binder"],
    [
        "anbox-vndbinder",
        "vndpuddlejumper",
        "vndbonder",
        "vndbinder",
    ],
    ["anbox-hwbinder", "hwpuddlejumper", "hwbonder", "hwbinder"],
];
const CONFIG_KEYS: [&str; 3] = ["binder", "vndbinder", "hwbinder"];

/// Stands in for `waydroid` and `systemctl`. `waydroid init` and `upgrade`
/// do what Waydroid 1.6.3 does to `waydroid.cfg` and `config_nodes`, and init
/// fails if it would have to download an image.
pub struct FakeHost {
    paths: Paths,
    pub calls: RefCell<Vec<String>>,
    /// Images init copied from a `file://` channel entry instead of skipping.
    pub fetched: RefCell<Vec<String>>,
    /// Makes the next `waydroid init` die as if systemd stopped the unit.
    pub interrupt_init: Cell<bool>,
}

impl FakeHost {
    pub fn new(paths: &Paths) -> Self {
        Self {
            paths: paths.clone(),
            calls: RefCell::new(Vec::new()),
            fetched: RefCell::new(Vec::new()),
            interrupt_init: Cell::new(false),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    fn init(&self, args: &[String]) -> anyhow::Result<()> {
        if self.interrupt_init.take() {
            return Err(android_setup::host::Interrupted {
                program: "waydroid".into(),
                signal: 15,
            }
            .into());
        }
        let value = |flag: &str| -> anyhow::Result<&str> {
            let index = args
                .iter()
                .position(|arg| arg == flag)
                .context("missing init flag")?;
            args.get(index + 1)
                .map(String::as_str)
                .context("missing init value")
        };
        let cfg_path = self.paths.waydroid_cfg();
        let mut config = WaydroidConfig::load(&cfg_path)?;
        let system_ota = format!(
            "{}/{}/waydroid_x86_64/{}.json",
            value("-c")?,
            value("-r")?,
            value("-s")?
        );
        let vendor_ota = format!("{}/waydroid_x86_64/MAINLINE.json", value("-v")?);
        for (key, url, datetime) in [
            ("system_ota", &system_ota, "system_datetime"),
            ("vendor_ota", &vendor_ota, "vendor_datetime"),
        ] {
            let seeded = config.get("waydroid", key).map(|value| value.into_owned());
            ensure!(
                seeded.as_deref() == Some(url.as_str()),
                "{key} {seeded:?} is not {url}: init resets the build time"
            );
            let channel: bundle::OtaChannel =
                serde_json::from_slice(&fs::read(url.trim_start_matches("file://"))?)?;
            let known: u64 = config
                .get("waydroid", datetime)
                .context("no build time")?
                .parse()?;
            ensure!(!channel.response.is_empty(), "{url} is empty");
            for entry in channel
                .response
                .iter()
                .filter(|entry| entry.datetime > known)
            {
                let Some(zip) = entry.url.strip_prefix("file://") else {
                    bail!("waydroid init would download {}", entry.url);
                };
                ensure!(Path::new(zip).is_file(), "{zip} is not staged");
                self.fetched.borrow_mut().push(entry.filename.clone());
            }
        }
        config.set("waydroid", "arch", "x86_64")?;
        config.set("waydroid", "vendor_type", "MAINLINE")?;
        config.set("waydroid", "images_path", value("-i")?)?;
        let dev = self.paths.dev();
        for (key, names) in CONFIG_KEYS.iter().zip(BINDER_NAMES) {
            let found = names
                .iter()
                .rev()
                .find(|name| dev.join(name).exists())
                .context("no binder node")?;
            config.set("waydroid", key, found)?;
        }
        config.save(&cfg_path)?;
        self.write_config_nodes(&config)
    }

    fn write_config_nodes(&self, config: &WaydroidConfig) -> anyhow::Result<()> {
        let mut lines = vec!["lxc.mount.entry = tmpfs dev tmpfs nosuid 0 0".to_owned()];
        for key in CONFIG_KEYS {
            let source = config.get("waydroid", key).context("no binder name")?;
            lines.push(format!(
                "lxc.mount.entry = /dev/{source} dev/{key} none bind,create=file,optional 0 0"
            ));
        }
        let path = self.paths.config_nodes();
        fs::create_dir_all(path.parent().context("no parent")?)?;
        fs::write(path, lines.join("\n") + "\n")?;
        Ok(())
    }
}

impl Host for FakeHost {
    fn waydroid(&self, args: &[&OsStr]) -> anyhow::Result<()> {
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        self.calls
            .borrow_mut()
            .push(format!("waydroid {}", args.join(" ")));
        match args.first().map(String::as_str) {
            Some("init") => self.init(&args),
            Some("upgrade") => {
                self.write_config_nodes(&WaydroidConfig::load(&self.paths.waydroid_cfg())?)
            }
            _ => Ok(()),
        }
    }

    fn systemctl(&self, args: &[&str]) -> anyhow::Result<()> {
        self.calls
            .borrow_mut()
            .push(format!("systemctl {}", args.join(" ")));
        Ok(())
    }
}

pub fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
