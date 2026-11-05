use android_protocol::APP_ID_PREFIX;

const DESKTOP_SUFFIX: &str = ".desktop";

/// Whether `name` is a well-formed Android package name: dot-separated
/// segments of ASCII letters, digits and underscores, each starting with a
/// letter. Nothing else ever reaches an `Exec` line or the agent.
pub fn is_package_name(name: &str) -> bool {
    name.contains('.')
        && name.split('.').all(|segment| {
            segment.starts_with(|first: char| first.is_ascii_alphabetic())
                && segment
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || char == '_')
        })
}

/// The package of a `waydroid.<package>.desktop` file name.
pub fn package_of_file(file_name: &str) -> Option<&str> {
    file_name
        .strip_prefix(APP_ID_PREFIX)?
        .strip_suffix(DESKTOP_SUFFIX)
        .filter(|package| is_package_name(package))
}

/// The file name of the desktop entry of `package`.
pub fn file_of_package(package: &str) -> String {
    format!("{APP_ID_PREFIX}{package}{DESKTOP_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_real_package_names() {
        assert!(is_package_name("com.whatsapp"));
        assert!(is_package_name("org.lineageos.jelly"));
        assert!(is_package_name("com.example.my_app2"));
    }

    #[test]
    fn rejects_anything_that_could_escape_an_exec_line() {
        assert!(!is_package_name("whatsapp"));
        assert!(!is_package_name("com.whats app"));
        assert!(!is_package_name("com.x;rm -rf ~"));
        assert!(!is_package_name("com..x"));
        assert!(!is_package_name("com.9x"));
    }

    #[test]
    fn maps_file_names_to_packages() {
        assert_eq!(
            package_of_file("waydroid.com.whatsapp.desktop"),
            Some("com.whatsapp")
        );
        assert_eq!(package_of_file("Waydroid.desktop"), None);
        assert_eq!(package_of_file("waydroid.com.x.desktop~"), None);
        assert_eq!(
            file_of_package("com.whatsapp"),
            "waydroid.com.whatsapp.desktop"
        );
    }
}
