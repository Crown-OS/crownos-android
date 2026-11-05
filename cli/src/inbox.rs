//! Copying an APK where crown-agent may install it from.

use std::fs::{self, DirBuilder};
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

const PRIVATE: u32 = 0o700;
const FILE_URI: &str = "file://";
const APK_SUFFIX: &str = ".apk";

/// Copies `apk` into `inbox` under a fresh name, since the agent deletes
/// each file after its install attempt, and returns that name.
pub fn copy_in(apk: &str, inbox: &Path) -> Result<String> {
    let source = Path::new(apk.strip_prefix(FILE_URI).unwrap_or(apk));
    if !source.is_file() {
        anyhow::bail!("{} is not a file", source.display());
    }
    DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE)
        .create(inbox)
        .with_context(|| format!("could not create {}", inbox.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let name = format!("{}-{stamp}{APK_SUFFIX}", plain_stem(source));
    fs::copy(source, inbox.join(&name)).with_context(|| {
        format!(
            "could not copy {} into {}",
            source.display(),
            inbox.display()
        )
    })?;
    Ok(name)
}

/// The file stem with everything but letters, digits, `.`, `_` and `-`
/// replaced, so the name stays a plain file name.
fn plain_stem(source: &Path) -> String {
    let stem: String = source
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_default()
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '-') {
                char
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim_start_matches('.');
    if stem.is_empty() {
        "app".to_owned()
    } else {
        stem.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_plain() {
        assert_eq!(plain_stem(Path::new("/tmp/My App (1).apk")), "My_App__1_");
        assert_eq!(plain_stem(Path::new("/tmp/..apk")), "app");
    }

    #[test]
    fn copies_into_a_private_inbox() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("cli-inbox-{}", std::process::id()));
        fs::create_dir_all(&root)?;
        let apk = root.join("notes.apk");
        fs::write(&apk, "apk")?;
        let inbox = root.join("data/crownos/inbox");
        let name = copy_in(&format!("file://{}", apk.display()), &inbox)?;
        assert!(name.starts_with("notes-") && name.ends_with(".apk"));
        assert_eq!(fs::read_to_string(inbox.join(&name))?, "apk");
        assert_eq!(fs::metadata(&inbox)?.permissions().mode() & 0o777, PRIVATE);
        assert!(copy_in(&root.join("missing.apk").display().to_string(), &inbox).is_err());
        fs::remove_dir_all(&root)?;
        Ok(())
    }
}
