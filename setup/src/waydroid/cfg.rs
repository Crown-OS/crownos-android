use std::borrow::Cow;
use std::io;
use std::path::Path;

use crate::util::atomic;

/// A key or value Python's configparser would reject or silently change.
#[derive(Debug, thiserror::Error)]
pub enum CfgError {
    #[error("{0:?} is not a valid waydroid.cfg key")]
    Key(String),
    #[error("the value of {0} spans lines")]
    Value(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    key: String,
    raw_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Section {
    name: String,
    entries: Vec<Entry>,
}

/// `waydroid.cfg`, which Waydroid reads with Python's `configparser`. Keys keep
/// their case, but match case-insensitively as configparser folds them, so a
/// rewritten file never holds a duplicate that configparser rejects. Values
/// are stored with `%` escaped for configparser's interpolation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaydroidConfig {
    sections: Vec<Section>,
}

impl WaydroidConfig {
    /// The file at `path`, or an empty config when there is none yet.
    pub fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        atomic::write(path, self.render().as_bytes())
    }

    /// Loads the file at `path`, applies `change` and saves it.
    pub fn edit(
        path: &Path,
        change: impl FnOnce(&mut Self) -> Result<(), CfgError>,
    ) -> anyhow::Result<()> {
        let mut config = Self::load(path)?;
        change(&mut config)?;
        Ok(config.save(path)?)
    }

    /// Reads the subset of configparser syntax Waydroid writes. Comments go;
    /// repeated sections and keys merge, the last value winning.
    pub fn parse(text: &str) -> Self {
        let mut config = Self::default();
        let mut current: Option<usize> = None;
        let mut last_key: Option<usize> = None;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(['#', ';']) {
                continue;
            }
            if line.starts_with(char::is_whitespace)
                && let (Some(section), Some(entry)) = (current, last_key)
            {
                let raw = &mut config.sections[section].entries[entry].raw_value;
                raw.push('\n');
                raw.push_str(trimmed);
                continue;
            }
            if let Some(name) = trimmed
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                current = Some(config.section_index(name));
                last_key = None;
                continue;
            }
            let (Some(section), Some(split)) = (current, trimmed.find(['=', ':'])) else {
                continue;
            };
            let key = trimmed[..split].trim_end();
            let raw_value = trimmed[split + 1..].trim_start();
            last_key = Some(config.sections[section].put(key, raw_value.to_owned()));
        }
        config
    }

    pub fn render(&self) -> String {
        let mut text = String::new();
        for section in &self.sections {
            text.push('[');
            text.push_str(&section.name);
            text.push_str("]\n");
            for entry in &section.entries {
                text.push_str(&entry.key);
                text.push_str(" = ");
                text.push_str(&entry.raw_value.replace('\n', "\n\t"));
                text.push('\n');
            }
            text.push('\n');
        }
        text
    }

    /// The value of `key`, with configparser's `%%` escapes undone.
    pub fn get(&self, section: &str, key: &str) -> Option<Cow<'_, str>> {
        let raw = &self.section(section)?.entry(key)?.raw_value;
        Some(if raw.contains('%') {
            Cow::Owned(raw.replace("%%", "%"))
        } else {
            Cow::Borrowed(raw)
        })
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) -> Result<(), CfgError> {
        if !is_valid_key(key) {
            return Err(CfgError::Key(key.to_owned()));
        }
        if value.contains(['\n', '\r']) {
            return Err(CfgError::Value(key.to_owned()));
        }
        let index = self.section_index(section);
        self.sections[index].put(key, value.trim().replace('%', "%%"));
        Ok(())
    }

    pub fn remove(&mut self, section: &str, key: &str) {
        if let Some(section) = self.sections.iter_mut().find(|found| found.name == section) {
            section
                .entries
                .retain(|entry| !entry.key.eq_ignore_ascii_case(key));
        }
    }

    fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }

    fn section_index(&mut self, name: &str) -> usize {
        self.sections
            .iter()
            .position(|section| section.name == name)
            .unwrap_or_else(|| {
                self.sections.push(Section {
                    name: name.to_owned(),
                    entries: Vec::new(),
                });
                self.sections.len() - 1
            })
    }
}

impl Section {
    fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.key.eq_ignore_ascii_case(key))
    }

    fn put(&mut self, key: &str, raw_value: String) -> usize {
        match self
            .entries
            .iter()
            .position(|entry| entry.key.eq_ignore_ascii_case(key))
        {
            Some(index) => {
                self.entries[index] = Entry {
                    key: key.to_owned(),
                    raw_value,
                };
                index
            }
            None => {
                self.entries.push(Entry {
                    key: key.to_owned(),
                    raw_value,
                });
                self.entries.len() - 1
            }
        }
    }
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-@".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WRITTEN_BY_WAYDROID: &str = "[waydroid]
arch = x86_64
vendor_type = MAINLINE
binder = anbox-binder
system_ota = https://ota.waydro.id/system/lineage/waydroid_x86_64/GAPPS.json

[properties]
persist.waydroid.multi_windows = false

";

    #[test]
    fn an_untouched_file_renders_back_the_same() {
        let config = WaydroidConfig::parse(WRITTEN_BY_WAYDROID);
        assert_eq!(config.render(), WRITTEN_BY_WAYDROID);
    }

    #[test]
    fn setting_replaces_in_place_and_appends_new_keys() -> Result<(), CfgError> {
        let mut config = WaydroidConfig::parse(WRITTEN_BY_WAYDROID);
        config.set("waydroid", "binder", "binder")?;
        config.set("waydroid", "drm_device", "/dev/dri/renderD128")?;
        config.set("properties", "persist.waydroid.multi_windows", "true")?;
        let text = config.render();
        assert!(text.contains("binder = binder\nsystem_ota"));
        assert!(text.contains("system_ota = https://ota.waydro.id/system/lineage/waydroid_x86_64/GAPPS.json\ndrm_device = /dev/dri/renderD128\n"));
        assert!(text.contains("[properties]\npersist.waydroid.multi_windows = true\n"));
        Ok(())
    }

    #[test]
    fn keys_keep_their_case_but_never_duplicate_under_configparser_folding() -> Result<(), CfgError>
    {
        let mut config = WaydroidConfig::parse("[properties]\nro.foo.Bar = 1\n");
        config.set("properties", "ro.foo.bar", "2")?;
        assert_eq!(config.render(), "[properties]\nro.foo.bar = 2\n\n");
        config.set("properties", "ro.Camel.Case", "3")?;
        assert!(config.render().contains("ro.Camel.Case = 3"));
        Ok(())
    }

    #[test]
    fn percent_signs_survive_interpolation() -> Result<(), CfgError> {
        let mut config = WaydroidConfig::default();
        config.set("properties", "ro.test.percent", "50%")?;
        assert!(config.render().contains("ro.test.percent = 50%%"));
        let reread = WaydroidConfig::parse(&config.render());
        assert_eq!(
            reread.get("properties", "ro.test.percent").as_deref(),
            Some("50%")
        );
        Ok(())
    }

    #[test]
    fn keys_and_values_configparser_would_break_are_rejected() {
        let mut config = WaydroidConfig::default();
        for key in ["", "a = b", "a:b", "[x]", "#comment", "two words"] {
            assert!(config.set("properties", key, "1").is_err(), "{key:?}");
        }
        assert!(config.set("properties", "ro.ok", "line\nbreak").is_err());
        assert_eq!(config, WaydroidConfig::default());
    }

    #[test]
    fn duplicate_sections_and_comments_are_normalised() {
        let config =
            WaydroidConfig::parse("# head\n[waydroid]\na = 1\n; note\n[waydroid]\na = 2\nb: 3\n");
        assert_eq!(config.render(), "[waydroid]\na = 2\nb = 3\n\n");
    }

    #[test]
    fn removing_a_key_drops_only_that_key() {
        let mut config = WaydroidConfig::parse(WRITTEN_BY_WAYDROID);
        config.remove("waydroid", "BINDER");
        assert_eq!(config.get("waydroid", "binder"), None);
        assert_eq!(config.get("waydroid", "arch").as_deref(), Some("x86_64"));
    }
}
