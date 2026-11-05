//! The Android properties every bundle sets (contract 11).

use std::collections::BTreeMap;

/// Width and height of the fixed Android display: the atlas the agent packs
/// tasks into.
pub const DISPLAY_SIZE: u32 = 4096;

/// Contract 11's properties for bundle `version`.
pub fn bundle_props(version: &str) -> BTreeMap<String, String> {
    let display_size = DISPLAY_SIZE.to_string();
    [
        ("ro.crownos.bundle", version),
        ("persist.crownos.safe_mode", "0"),
        ("persist.waydroid.multi_windows", "true"),
        ("persist.waydroid.width", &display_size),
        ("persist.waydroid.height", &display_size),
        ("persist.wm.debug.caption_on_shell", "true"),
        ("persist.waydroid.suspend", "false"),
        ("debug.sf.nobootanimation", "1"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_version_and_the_atlas_size_are_set() {
        let props = bundle_props("2026.11.0");
        assert_eq!(props.len(), 8);
        assert_eq!(
            props.get("ro.crownos.bundle").map(String::as_str),
            Some("2026.11.0")
        );
        assert_eq!(
            props.get("persist.waydroid.width").map(String::as_str),
            Some("4096")
        );
    }
}
