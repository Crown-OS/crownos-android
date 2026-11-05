/// A parsed desktop entry: groups of `key=value` pairs in file order.
/// Comments and blank lines are dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopEntry {
    groups: Vec<(String, Vec<(String, String)>)>,
}

impl DesktopEntry {
    pub fn parse(text: &str) -> Self {
        let mut entry = Self::default();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                entry.groups.push((name.to_owned(), Vec::new()));
            } else if let (Some((key, value)), Some((_, pairs))) =
                (line.split_once('='), entry.groups.last_mut())
            {
                pairs.push((key.trim_end().to_owned(), value.trim_start().to_owned()));
            }
        }
        entry
    }

    /// The pairs of `group`, in file order.
    pub fn pairs(&self, group: &str) -> impl Iterator<Item = (&str, &str)> {
        self.groups
            .iter()
            .filter(move |(name, _)| name == group)
            .flat_map(|(_, pairs)| {
                pairs
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
            })
    }

    pub fn get(&self, group: &str, key: &str) -> Option<&str> {
        self.pairs(group)
            .find(|(found, _)| *found == key)
            .map(|(_, value)| value)
    }

    /// `key` and its localized variants, `key[xx]`.
    pub fn localized<'a>(
        &'a self,
        group: &'a str,
        key: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.pairs(group).filter(move |(found, _)| {
            found.strip_prefix(key).is_some_and(|rest| {
                rest.is_empty() || (rest.starts_with('[') && rest.ends_with(']'))
            })
        })
    }

    pub fn has_group(&self, group: &str) -> bool {
        self.groups.iter().any(|(name, _)| name == group)
    }

    /// Appends a group; pairs come from [`Self::set`].
    pub fn add_group(&mut self, group: &str) {
        self.groups.push((group.to_owned(), Vec::new()));
    }

    /// Adds `key=value` to the last group named `group`.
    pub fn set(&mut self, group: &str, key: &str, value: &str) {
        if let Some((_, pairs)) = self.groups.iter_mut().rev().find(|(name, _)| name == group) {
            pairs.push((key.to_owned(), value.to_owned()));
        }
    }

    pub fn render(&self) -> String {
        let mut text = String::new();
        for (index, (name, pairs)) in self.groups.iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            text.push_str(&format!("[{name}]\n"));
            for (key, value) in pairs {
                text.push_str(&format!("{key}={value}\n"));
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_groups_keys_and_localized_names() {
        let entry = DesktopEntry::parse(
            "# comment\n[Desktop Entry]\nName=Notes\nName[de]=Notizen\nNameX=no\n\n[Desktop Action a]\nName=A\n",
        );
        assert_eq!(entry.get("Desktop Entry", "Name"), Some("Notes"));
        assert_eq!(entry.get("Desktop Action a", "Name"), Some("A"));
        let names: Vec<_> = entry.localized("Desktop Entry", "Name").collect();
        assert_eq!(names, [("Name", "Notes"), ("Name[de]", "Notizen")]);
    }

    #[test]
    fn renders_what_it_parses() {
        let text = "[Desktop Entry]\nName=Notes\n\n[Desktop Action a]\nExec=x\n";
        assert_eq!(DesktopEntry::parse(text).render(), text);
    }
}
