use std::collections::BTreeMap;

/// The numeric `key value` pairs of a `/proc` or cgroup file, as in
/// `usage_usec 42`, `MemAvailable:  1024 kB` or `Pss:  12 kB`. Other lines are skipped.
pub fn pairs(text: &str) -> impl Iterator<Item = (&str, u64)> {
    text.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let key = fields.next()?.trim_end_matches(':');
        let value = fields.next()?.parse().ok()?;
        Some((key, value))
    })
}

pub fn value_of(text: &str, key: &str) -> Option<u64> {
    pairs(text).find_map(|(name, value)| (name == key).then_some(value))
}

/// Every pair, for files like `memory.stat` that results keep whole.
pub fn all_pairs(text: &str) -> BTreeMap<String, u64> {
    pairs(text)
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMORY_STAT: &str = include_str!("../../fixtures/memory.stat");
    const MEMINFO: &str = include_str!("../../fixtures/meminfo");
    const EVENTS: &str = include_str!("../../fixtures/cgroup.events");

    #[test]
    fn reads_memory_stat() {
        let stat = all_pairs(MEMORY_STAT);
        assert_eq!(stat.len(), MEMORY_STAT.lines().count());
        assert_eq!(stat.get("anon"), Some(&317_427_712));
        assert_eq!(stat.get("file"), Some(&1_327_943_680));
        assert_eq!(stat.get("kernel_stack"), Some(&1_753_088));
    }

    #[test]
    fn reads_meminfo_and_cgroup_events() {
        assert!(value_of(MEMINFO, "MemAvailable").is_some_and(|kib| kib > 0));
        assert!(value_of(MEMINFO, "MemTotal") >= value_of(MEMINFO, "MemAvailable"));
        assert_eq!(value_of(EVENTS, "frozen"), Some(0));
        assert_eq!(value_of(EVENTS, "populated"), Some(1));
        assert_eq!(value_of(EVENTS, "missing"), None);
    }
}
