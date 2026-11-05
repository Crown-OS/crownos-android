use android_protocol::app_id;

use crate::{DesktopEntry, MAIN_GROUP, MIRROR_MARKER, is_package_name};

/// The action Waydroid adds for an app's Android settings page.
const SETTINGS_ACTION: &str = "app-settings";
const KEPT_KEYS: [&str; 3] = ["Icon", "Categories", "NoDisplay"];

/// The menu entry for `package`, built from the entry Waydroid wrote. Name,
/// Icon, Categories and NoDisplay are kept; the app launches through
/// `crown-android` and its windows group under `waydroid.<package>`. `None`
/// when the source has no name or the package name is malformed.
pub fn mirror_entry(source: &str, package: &str) -> Option<String> {
    if !is_package_name(package) {
        return None;
    }
    let source = DesktopEntry::parse(source);
    source.get(MAIN_GROUP, "Name")?;
    let mut mirror = DesktopEntry::default();
    mirror.add_group(MAIN_GROUP);
    mirror.set(MAIN_GROUP, "Type", "Application");
    for (key, value) in source.localized(MAIN_GROUP, "Name") {
        mirror.set(MAIN_GROUP, key, value);
    }
    for key in KEPT_KEYS {
        if let Some(value) = source.get(MAIN_GROUP, key) {
            mirror.set(MAIN_GROUP, key, value);
        }
    }
    mirror.set(
        MAIN_GROUP,
        "Exec",
        &format!("crown-android launch {package}"),
    );
    mirror.set(MAIN_GROUP, "StartupWMClass", &app_id(package));
    mirror.set(MAIN_GROUP, MIRROR_MARKER, package);
    let settings_group = format!("Desktop Action {SETTINGS_ACTION}");
    if source.has_group(&settings_group) {
        mirror.set(MAIN_GROUP, "Actions", &format!("{SETTINGS_ACTION};"));
        mirror.add_group(&settings_group);
        for (key, value) in source.localized(&settings_group, "Name") {
            mirror.set(&settings_group, key, value);
        }
        if let Some(icon) = source.get(&settings_group, "Icon") {
            mirror.set(&settings_group, "Icon", icon);
        }
        mirror.set(
            &settings_group,
            "Exec",
            &format!("crown-android settings {package}"),
        );
    }
    Some(mirror.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What Waydroid's user manager writes for an app.
    const WAYDROID_ENTRY: &str = "\
[Desktop Entry]
Type=Application
Name=WhatsApp
Name[de]=WhatsApp DE
Exec=waydroid app launch com.whatsapp
Icon=/home/ada/.local/share/waydroid/data/icons/com.whatsapp.png
Categories=X-WayDroid-App;Network;
X-Purism-FormFactor=Workstation;Mobile;
Actions=app-settings;

[Desktop Action app-settings]
Name=App Settings
Exec=waydroid app intent android.settings.APPLICATION_DETAILS_SETTINGS package:com.whatsapp
Icon=/home/ada/.local/share/waydroid/data/icons/com.android.settings.png
";

    const MIRRORED: &str = "\
[Desktop Entry]
Type=Application
Name=WhatsApp
Name[de]=WhatsApp DE
Icon=/home/ada/.local/share/waydroid/data/icons/com.whatsapp.png
Categories=X-WayDroid-App;Network;
Exec=crown-android launch com.whatsapp
StartupWMClass=waydroid.com.whatsapp
X-CrownOS-Android=com.whatsapp
Actions=app-settings;

[Desktop Action app-settings]
Name=App Settings
Icon=/home/ada/.local/share/waydroid/data/icons/com.android.settings.png
Exec=crown-android settings com.whatsapp
";

    #[test]
    fn rewrites_launch_and_settings_through_crown_android() {
        assert_eq!(
            mirror_entry(WAYDROID_ENTRY, "com.whatsapp").as_deref(),
            Some(MIRRORED)
        );
    }

    #[test]
    fn keeps_no_display_for_system_apps() {
        let source = "[Desktop Entry]\nName=Settings\nNoDisplay=true\n";
        let mirrored = mirror_entry(source, "com.android.settings").unwrap_or_default();
        assert!(mirrored.contains("\nNoDisplay=true\n"));
        assert!(!mirrored.contains("Actions="));
    }

    #[test]
    fn refuses_entries_without_a_name_or_with_a_bad_package() {
        assert_eq!(mirror_entry("[Desktop Entry]\nIcon=x\n", "com.x"), None);
        assert_eq!(mirror_entry(WAYDROID_ENTRY, "com.x;reboot"), None);
    }
}
