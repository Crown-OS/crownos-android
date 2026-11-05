/// Whether `text` is usable as one path component: not empty, not `.` or
/// `..`, and without a separator or NUL.
pub fn is_file_name(text: &str) -> bool {
    !matches!(text, "" | "." | "..") && !text.contains(['/', '\0'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_single_components_pass() {
        assert!(is_file_name(
            "lineage-20.0-20260403-GAPPS-waydroid_x86_64-system.zip"
        ));
        for hostile in ["", ".", "..", "../x", "a/b", "nul\0"] {
            assert!(!is_file_name(hostile), "{hostile:?}");
        }
    }
}
