//! `crown-android diagnose`: one `tar.zst` with the daemon's state, the unit
//! journals, Waydroid's log and the bundle and image versions.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use runtime::host::{CONTAINER_SERVICE, SESSION_SERVICE, SETUP_SERVICE};

use crate::util::time::utc_stamp;

const ZSTD_LEVEL: i32 = 9;
/// The tail of each log that goes in, so an archive stays small.
const LOG_TAIL: u64 = 4 * 1024 * 1024;
const JOURNAL_LINES: &str = "5000";
const DAEMON_SERVICE: &str = "crown-androidd.service";
/// System files copied in, relative to the system root.
const SYSTEM_FILES: [(&str, &str); 4] = [
    ("waydroid.log", "var/lib/waydroid/waydroid.log"),
    ("waydroid.cfg", "var/lib/waydroid/waydroid.cfg"),
    ("android.ron", "etc/crownos/android.ron"),
    ("setup.json", "var/lib/crownos/android/setup.json"),
];
/// Keys of `waydroid.cfg` that name the images in use.
const IMAGE_KEYS: [&str; 5] = [
    "system_datetime",
    "vendor_datetime",
    "images_path",
    "system_ota",
    "vendor_ota",
];

/// What one archive needs.
pub struct Report {
    /// The daemon's state, already rendered.
    pub state: String,
    pub archive_dir: PathBuf,
    pub system_root: PathBuf,
}

/// Which journal a unit logs to.
#[derive(Clone, Copy)]
enum Journal {
    User,
    System,
}

pub fn write(report: &Report) -> io::Result<PathBuf> {
    fs::create_dir_all(&report.archive_dir)?;
    let now = SystemTime::now();
    let path = report
        .archive_dir
        .join(format!("diagnose-{}.tar.zst", utc_stamp(now)));
    let encoder = zstd::Encoder::new(File::create(&path)?, ZSTD_LEVEL)?;
    let mut archive = tar::Builder::new(encoder);
    let mtime = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let mut add = |name: &str, bytes: &[u8]| -> io::Result<()> {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(mtime);
        header.set_cksum();
        archive.append_data(&mut header, name, bytes)
    };
    add("state.txt", report.state.as_bytes())?;
    add("versions.txt", versions(&report.system_root).as_bytes())?;
    for (unit, journal) in [
        (DAEMON_SERVICE, Journal::User),
        (SESSION_SERVICE, Journal::User),
        (venus::RENDER_SERVICE, Journal::User),
        (CONTAINER_SERVICE, Journal::System),
        (SETUP_SERVICE, Journal::System),
    ] {
        add(&format!("journal/{unit}.txt"), &journal_of(unit, journal))?;
    }
    for (name, relative) in SYSTEM_FILES {
        add(name, &tail(&report.system_root.join(relative)))?;
    }
    archive.into_inner()?.finish()?;
    Ok(path)
}

fn journal_of(unit: &str, journal: Journal) -> Vec<u8> {
    let mut command = Command::new("journalctl");
    if let Journal::User = journal {
        command.arg("--user");
    }
    command.args([
        "-u",
        unit,
        "-b",
        "--no-pager",
        "-o",
        "short-iso",
        "-n",
        JOURNAL_LINES,
    ]);
    match command.output() {
        Ok(output) => [output.stdout, output.stderr].concat(),
        Err(error) => format!("journalctl could not run: {error}\n").into_bytes(),
    }
}

/// The last [`LOG_TAIL`] bytes of `path`, or why it could not be read.
fn tail(path: &Path) -> Vec<u8> {
    let read = || -> io::Result<Vec<u8>> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL)))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    read().unwrap_or_else(|error| format!("{}: {error}\n", path.display()).into_bytes())
}

/// The daemon version, the installed bundle and the images Waydroid runs.
fn versions(system_root: &Path) -> String {
    let config = runtime::installation::Installation::load(system_root);
    let waydroid_cfg =
        fs::read_to_string(system_root.join("var/lib/waydroid/waydroid.cfg")).unwrap_or_default();
    let images: String = waydroid_cfg
        .lines()
        .filter(|line| {
            line.split_once('=')
                .is_some_and(|(key, _)| IMAGE_KEYS.contains(&key.trim()))
        })
        .map(|line| format!("{}\n", line.trim()))
        .collect();
    format!(
        "crown-androidd {}\ninstallation: {config:?}\n{images}",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_readable_archive() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("diagnose-{}", std::process::id()));
        fs::create_dir_all(root.join("var/lib/waydroid"))?;
        fs::write(
            root.join("var/lib/waydroid/waydroid.cfg"),
            "[waydroid]\nsystem_datetime = 1775208984\nmount_overlays = True\n",
        )?;
        let report = Report {
            state: "state: Stopped\n".into(),
            archive_dir: root.join("cache"),
            system_root: root.clone(),
        };
        let path = write(&report)?;
        assert!(
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("diagnose-"))
        );
        let decoder = zstd::Decoder::new(File::open(&path)?)?;
        let mut names = Vec::new();
        let mut versions = String::new();
        for entry in tar::Archive::new(decoder).entries()? {
            let mut entry = entry?;
            let name = entry.path()?.display().to_string();
            if name == "versions.txt" {
                entry.read_to_string(&mut versions)?;
            }
            names.push(name);
        }
        assert!(names.contains(&"state.txt".to_owned()));
        assert!(names.contains(&"journal/waydroid-container.service.txt".to_owned()));
        assert!(names.contains(&"waydroid.log".to_owned()));
        assert!(versions.contains("system_datetime = 1775208984"));
        assert!(!versions.contains("mount_overlays"));
        fs::remove_dir_all(&root)
    }
}
