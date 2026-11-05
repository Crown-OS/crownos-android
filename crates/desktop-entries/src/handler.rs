/// A desktop entry `crown-android` installs for itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handler {
    pub file_name: &'static str,
    pub contents: &'static str,
}

/// Opens APK files and `market:` links through Android. Each MIME type needs
/// its own `Exec`, so each gets its own entry.
pub const HANDLERS: [Handler; 2] = [
    Handler {
        file_name: "crown-android-handler.desktop",
        contents: "\
[Desktop Entry]
Type=Application
Name=Android App Installer
Comment=Install Android apps
Icon=application-vnd.android.package-archive
Exec=crown-android install %f
MimeType=application/vnd.android.package-archive;
NoDisplay=true
",
    },
    Handler {
        file_name: "crown-android-market-handler.desktop",
        contents: "\
[Desktop Entry]
Type=Application
Name=Google Play Links
Comment=Open market links in Android
Icon=application-vnd.android.package-archive
Exec=crown-android open %u
MimeType=x-scheme-handler/market;
NoDisplay=true
",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DesktopEntry, MAIN_GROUP};

    #[test]
    fn apks_install_and_market_links_open() {
        let exec_for = |mime: &str| {
            HANDLERS.iter().find_map(|handler| {
                let entry = DesktopEntry::parse(handler.contents);
                entry
                    .get(MAIN_GROUP, "MimeType")
                    .is_some_and(|types| types.split(';').any(|found| found == mime))
                    .then(|| entry.get(MAIN_GROUP, "Exec").map(str::to_owned))
                    .flatten()
            })
        };
        assert_eq!(
            exec_for("application/vnd.android.package-archive").as_deref(),
            Some("crown-android install %f")
        );
        assert_eq!(
            exec_for("x-scheme-handler/market").as_deref(),
            Some("crown-android open %u")
        );
    }
}
