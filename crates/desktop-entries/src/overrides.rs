//! Per-user overrides that hide the entries the waydroid package installs in
//! `/usr/share/applications`: its full-UI launcher and its APK and `market:`
//! handlers, which would compete with Android apps and with ours.

use std::fs;
use std::io;
use std::path::Path;

use crate::util::atomic;
use crate::{DesktopEntry, MAIN_GROUP};

/// The entries `pacman -Ql waydroid` lists under `/usr/share/applications`.
pub const WAYDROID_ENTRIES: [&str; 3] = [
    "Waydroid.desktop",
    "waydroid.app.install.desktop",
    "waydroid.market.desktop",
];

/// Key every override carries, so only ours are ever removed.
const OVERRIDE_MARKER: &str = "X-CrownOS-Android-Hides";

/// An entry that shadows the system entry `file_name` and hides it.
pub fn hidden_override(file_name: &str) -> String {
    format!(
        "[{MAIN_GROUP}]\nType=Application\nName=Waydroid\nHidden=true\n{OVERRIDE_MARKER}={file_name}\n"
    )
}

/// Writes the overrides into `target_dir`, leaving alone any file of the
/// same name the user wrote.
pub fn hide_waydroid_entries(target_dir: &Path) -> io::Result<()> {
    for file_name in WAYDROID_ENTRIES {
        let path = target_dir.join(file_name);
        let wanted = hidden_override(file_name);
        match fs::read_to_string(&path) {
            Ok(text) if text == wanted => {}
            Ok(text) if !is_override(&text) => {
                log::info!("keeping the user's own {}", path.display());
            }
            _ => atomic::write(&path, wanted.as_bytes())?,
        }
    }
    Ok(())
}

/// Deletes the overrides from `target_dir`, so Waydroid's entries show again.
pub fn unhide_waydroid_entries(target_dir: &Path) -> io::Result<()> {
    for file_name in WAYDROID_ENTRIES {
        let path = target_dir.join(file_name);
        if fs::read_to_string(&path).is_ok_and(|text| is_override(&text)) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn is_override(text: &str) -> bool {
    DesktopEntry::parse(text)
        .get(MAIN_GROUP, OVERRIDE_MARKER)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_hides_the_system_entry() {
        let entry = DesktopEntry::parse(&hidden_override("Waydroid.desktop"));
        assert_eq!(entry.get(MAIN_GROUP, "Hidden"), Some("true"));
        assert_eq!(
            entry.get(MAIN_GROUP, OVERRIDE_MARKER),
            Some("Waydroid.desktop")
        );
    }

    #[test]
    fn overrides_come_and_go_without_touching_the_users_files() -> io::Result<()> {
        let dir = std::env::temp_dir().join(format!("desktop-overrides-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let users_own = dir.join(WAYDROID_ENTRIES[2]);
        fs::write(&users_own, "[Desktop Entry]\nName=Mine\n")?;
        hide_waydroid_entries(&dir)?;
        assert_eq!(
            fs::read_to_string(dir.join(WAYDROID_ENTRIES[0]))?,
            hidden_override(WAYDROID_ENTRIES[0])
        );
        assert_eq!(
            fs::read_to_string(&users_own)?,
            "[Desktop Entry]\nName=Mine\n"
        );
        unhide_waydroid_entries(&dir)?;
        assert!(!dir.join(WAYDROID_ENTRIES[0]).exists());
        assert!(users_own.exists());
        fs::remove_dir_all(&dir)
    }
}
