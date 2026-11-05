use std::fs;
use std::io;
use std::path::PathBuf;

use android_setup::Paths;
use android_setup::bundle_install::shipped;
use android_setup::props;
use bundle::{Arch, Flavor, ImagePin, Manifest, OtaEntry, OverlayFile, OverlayPath};
use crownos_config::system::AndroidConfig;

use super::sign::TestKey;
use super::{Scratch, sha256_hex};

pub const AT_PAGESZ: u64 = 6;
pub const BUNDLE_VERSION: &str = "2026.11.0";

/// A fake installed system under a temp dir: procfs with binderfs mounted,
/// a sysfs with one AMD GPU, and an empty `/var/lib/waydroid`.
pub struct Machine {
    pub root: Scratch,
    pub paths: Paths,
    pub key: TestKey,
}

impl Machine {
    pub fn new(label: &str) -> io::Result<Self> {
        let root = Scratch::new(label)?;
        let paths = Paths::under(&root);
        let machine = Self {
            root,
            paths,
            key: TestKey::new(7),
        };
        let proc_root = machine.paths.proc_root();
        fs::create_dir_all(proc_root.join("self"))?;
        fs::write(
            proc_root.join("filesystems"),
            "nodev\tsysfs\nnodev\tbinder\n\text4\n",
        )?;
        machine.set_page_size(4096)?;
        let binderfs = machine.paths.dev().join("binderfs");
        fs::create_dir_all(&binderfs)?;
        for node in ["binder-control", "binder", "vndbinder", "hwbinder"] {
            fs::write(binderfs.join(node), "")?;
        }
        fs::write(
            proc_root.join("self/mountinfo"),
            format!(
                "41 25 0:39 / {} rw,relatime shared:20 - binder binder rw\n",
                binderfs.display()
            ),
        )?;
        machine.add_card("card0", "amdgpu", true, "renderD128")?;
        fs::create_dir_all(machine.paths.overlay())?;
        fs::create_dir_all(machine.root.join("etc"))?;
        Ok(machine)
    }

    pub fn set_page_size(&self, size: u64) -> io::Result<()> {
        let auxv: Vec<u8> = [(AT_PAGESZ, size), (0, 0)]
            .iter()
            .flat_map(|(key, value)| [key.to_ne_bytes(), value.to_ne_bytes()])
            .flatten()
            .collect();
        fs::write(self.paths.proc_root().join("self/auxv"), auxv)
    }

    pub fn add_card(
        &self,
        name: &str,
        driver: &str,
        boot_vga: bool,
        render: &str,
    ) -> io::Result<()> {
        let device = self.paths.sys().join("class/drm").join(name).join("device");
        fs::create_dir_all(device.join("drm").join(render))?;
        fs::write(device.join("uevent"), format!("DRIVER={driver}\n"))?;
        fs::write(
            device.join("boot_vga"),
            if boot_vga { "1\n" } else { "0\n" },
        )
    }

    pub fn enable_android(&self, google_play: bool) -> anyhow::Result<()> {
        let config = AndroidConfig {
            enabled: true,
            google_play,
            tier: None,
            bundle: None,
        };
        android_setup::android_ron::save(&self.paths.android_ron(), &config)?;
        let marker = self.paths.pending_marker();
        fs::create_dir_all(
            marker
                .parent()
                .ok_or_else(|| anyhow::anyhow!("no parent"))?,
        )?;
        fs::write(marker, "")?;
        Ok(())
    }

    pub fn android_config(&self) -> anyhow::Result<Option<AndroidConfig>> {
        android_setup::android_ron::load(&self.paths.android_ron())
    }

    /// Writes `manifest` and `files` as a shipped bundle, signed with `self.key`.
    pub fn ship_bundle(
        &self,
        manifest: &Manifest,
        files: &[(&str, Vec<u8>)],
    ) -> anyhow::Result<PathBuf> {
        let dir = self.paths.shipped_bundles().join(&manifest.version);
        for (path, bytes) in files {
            let target = dir.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(target, bytes)?;
        }
        let bytes = serde_json::to_vec_pretty(manifest)?;
        fs::write(dir.join(shipped::MANIFEST), &bytes)?;
        fs::write(dir.join(shipped::SIGNATURE), self.key.sign(&bytes))?;
        Ok(dir)
    }

    pub fn read(&self, path: &str) -> io::Result<String> {
        fs::read_to_string(self.root.join(path))
    }
}

/// A manifest listing `files` with mode 0644, the contract 11 props and `images`.
pub fn manifest(
    version: &str,
    files: &[(&str, Vec<u8>)],
    images: Vec<ImagePin>,
) -> anyhow::Result<Manifest> {
    let files = files
        .iter()
        .map(|(path, bytes)| {
            Ok(OverlayFile {
                path: OverlayPath::try_from((*path).to_owned())?,
                sha256: sha256_hex(bytes),
                mode: 0o644,
            })
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(Manifest {
        format: bundle::FORMAT,
        version: version.to_owned(),
        waydroid: "1.6.3".into(),
        images,
        files,
        whiteouts: Vec::new(),
        props: props::bundle_props(version),
    })
}

/// An OTA entry for a zip served at `url`.
pub fn ota_entry(
    datetime: u64,
    filename: &str,
    romtype: &str,
    url: String,
    zip: &[u8],
) -> OtaEntry {
    OtaEntry {
        datetime,
        filename: filename.to_owned(),
        id: sha256_hex(zip),
        romtype: romtype.to_owned(),
        size: zip.len() as u64,
        url,
        version: "20.0".into(),
    }
}

pub fn pin(flavor: Flavor, system: OtaEntry, vendor: OtaEntry) -> ImagePin {
    ImagePin {
        flavor,
        arch: Arch::X86_64,
        system,
        vendor,
    }
}
