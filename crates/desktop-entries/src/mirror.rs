//! Keeping the user's menu in step with Waydroid's entries.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::package::file_of_package;
use crate::util::atomic;
use crate::{DesktopEntry, HANDLERS, MAIN_GROUP, MIRROR_MARKER, mirror_entry, package_of_file};

/// What one [`sync`] changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub written: Vec<String>,
    pub removed: Vec<String>,
}

impl SyncReport {
    pub fn changed(&self) -> bool {
        !self.written.is_empty() || !self.removed.is_empty()
    }
}

/// Mirrors every `waydroid.*.desktop` in `source_dir` into `target_dir` and
/// deletes the mirrors whose source is gone. Files are only rewritten when
/// their contents change.
pub fn sync(source_dir: &Path, target_dir: &Path) -> io::Result<SyncReport> {
    let mut report = SyncReport::default();
    let sources = package_files(source_dir)?;
    for (package, path) in &sources {
        let Some(mirrored) = fs::read_to_string(path)
            .ok()
            .and_then(|text| mirror_entry(&text, package))
        else {
            log::warn!("skipping unreadable desktop entry {}", path.display());
            continue;
        };
        let target = target_dir.join(file_of_package(package));
        if fs::read_to_string(&target).ok().as_deref() != Some(&mirrored) {
            atomic::write(&target, mirrored.as_bytes())?;
            report.written.push(package.clone());
        }
    }
    for (package, path) in package_files(target_dir)? {
        let ours = fs::read_to_string(&path).is_ok_and(|text| {
            DesktopEntry::parse(&text).get(MAIN_GROUP, MIRROR_MARKER) == Some(&package)
        });
        if ours && !sources.iter().any(|(source, _)| *source == package) {
            fs::remove_file(&path)?;
            report.removed.push(package);
        }
    }
    Ok(report)
}

/// Writes the APK and `market:` handler entries into `target_dir` when they
/// are missing or outdated.
pub fn install_handlers(target_dir: &Path) -> io::Result<()> {
    for handler in HANDLERS {
        let path = target_dir.join(handler.file_name);
        if fs::read_to_string(&path).ok().as_deref() != Some(handler.contents) {
            atomic::write(&path, handler.contents.as_bytes())?;
        }
    }
    Ok(())
}

/// Deletes every mirror and handler from `target_dir` and shows Waydroid's
/// own entries again, for when Android support is removed.
pub fn remove_all(target_dir: &Path) -> io::Result<()> {
    for (_, path) in package_files(target_dir)? {
        let ours = fs::read_to_string(&path).is_ok_and(|text| {
            DesktopEntry::parse(&text)
                .get(MAIN_GROUP, MIRROR_MARKER)
                .is_some()
        });
        if ours {
            fs::remove_file(&path)?;
        }
    }
    for handler in HANDLERS {
        match fs::remove_file(target_dir.join(handler.file_name)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    crate::unhide_waydroid_entries(target_dir)
}

/// What the splash shows for an app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    /// The PNG Waydroid exported, when the entry names one.
    pub icon: Option<PathBuf>,
}

/// Name and icon of `package` from Waydroid's entry in `source_dir`, or
/// `None` when Android does not know the package.
pub fn app_info(source_dir: &Path, package: &str) -> Option<AppInfo> {
    let text = fs::read_to_string(source_dir.join(file_of_package(package))).ok()?;
    let entry = DesktopEntry::parse(&text);
    Some(AppInfo {
        name: entry.get(MAIN_GROUP, "Name")?.to_owned(),
        icon: entry
            .get(MAIN_GROUP, "Icon")
            .map(PathBuf::from)
            .filter(|icon| icon.is_absolute()),
    })
}

fn package_files(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let package = package_of_file(name.to_str()?)?.to_owned();
            Some((package, entry.path()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dirs {
        root: PathBuf,
        source: PathBuf,
        target: PathBuf,
    }

    impl Dirs {
        fn new(name: &str) -> io::Result<Self> {
            let root = std::env::temp_dir().join(format!("desktop-{name}-{}", std::process::id()));
            let (source, target) = (root.join("source"), root.join("target"));
            fs::create_dir_all(&source)?;
            fs::create_dir_all(&target)?;
            Ok(Self {
                root,
                source,
                target,
            })
        }
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    const NOTES: &str = "[Desktop Entry]\nName=Notes\nIcon=/icons/org.example.notes.png\n";

    #[test]
    fn mirrors_new_entries_once() -> io::Result<()> {
        let dirs = Dirs::new("new")?;
        fs::write(
            dirs.source.join("waydroid.org.example.notes.desktop"),
            NOTES,
        )?;
        fs::write(dirs.source.join("Waydroid.desktop"), NOTES)?;
        let report = sync(&dirs.source, &dirs.target)?;
        assert_eq!(report.written, ["org.example.notes"]);
        let mirrored = fs::read_to_string(dirs.target.join("waydroid.org.example.notes.desktop"))?;
        assert!(mirrored.contains("Exec=crown-android launch org.example.notes"));
        assert!(!sync(&dirs.source, &dirs.target)?.changed());
        Ok(())
    }

    #[test]
    fn removes_a_mirror_when_its_source_goes_but_never_foreign_entries() -> io::Result<()> {
        let dirs = Dirs::new("removed")?;
        let source = dirs.source.join("waydroid.org.example.notes.desktop");
        fs::write(&source, NOTES)?;
        sync(&dirs.source, &dirs.target)?;
        let foreign = dirs.target.join("waydroid.org.example.other.desktop");
        fs::write(&foreign, NOTES)?;
        fs::remove_file(&source)?;
        let report = sync(&dirs.source, &dirs.target)?;
        assert_eq!(report.removed, ["org.example.notes"]);
        assert!(foreign.exists());
        Ok(())
    }

    #[test]
    fn handlers_are_installed_and_left_alone_when_current() -> io::Result<()> {
        let dirs = Dirs::new("handlers")?;
        install_handlers(&dirs.target)?;
        let path = dirs.target.join(HANDLERS[0].file_name);
        let written = fs::metadata(&path)?.modified()?;
        install_handlers(&dirs.target)?;
        assert_eq!(fs::metadata(&path)?.modified()?, written);
        Ok(())
    }

    #[test]
    fn removing_android_clears_everything_this_crate_wrote() -> io::Result<()> {
        let dirs = Dirs::new("remove-all")?;
        fs::write(
            dirs.source.join("waydroid.org.example.notes.desktop"),
            NOTES,
        )?;
        sync(&dirs.source, &dirs.target)?;
        install_handlers(&dirs.target)?;
        crate::hide_waydroid_entries(&dirs.target)?;
        remove_all(&dirs.target)?;
        assert_eq!(fs::read_dir(&dirs.target)?.count(), 0);
        Ok(())
    }

    #[test]
    fn app_info_reads_name_and_icon() -> io::Result<()> {
        let dirs = Dirs::new("info")?;
        fs::write(
            dirs.source.join("waydroid.org.example.notes.desktop"),
            NOTES,
        )?;
        assert_eq!(
            app_info(&dirs.source, "org.example.notes"),
            Some(AppInfo {
                name: "Notes".into(),
                icon: Some(PathBuf::from("/icons/org.example.notes.png")),
            })
        );
        assert_eq!(app_info(&dirs.source, "org.example.missing"), None);
        Ok(())
    }
}
