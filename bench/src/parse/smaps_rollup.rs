use serde::Serialize;

use super::pairs::pairs;

/// The fields of `/proc/<pid>/smaps_rollup` the budgets use, in KiB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SmapsRollup {
    pub rss_kib: u64,
    pub pss_kib: u64,
    pub pss_anon_kib: u64,
    pub pss_file_kib: u64,
    pub pss_shmem_kib: u64,
    pub swap_kib: u64,
    pub swap_pss_kib: u64,
}

impl SmapsRollup {
    /// `None` unless the text has a `Pss` line.
    pub fn parse(text: &str) -> Option<Self> {
        let mut rollup = Self::default();
        let mut has_pss = false;
        for (key, value) in pairs(text) {
            let field = match key {
                "Rss" => &mut rollup.rss_kib,
                "Pss" => {
                    has_pss = true;
                    &mut rollup.pss_kib
                }
                "Pss_Anon" => &mut rollup.pss_anon_kib,
                "Pss_File" => &mut rollup.pss_file_kib,
                "Pss_Shmem" => &mut rollup.pss_shmem_kib,
                "Swap" => &mut rollup.swap_kib,
                "SwapPss" => &mut rollup.swap_pss_kib,
                _ => continue,
            };
            *field = value;
        }
        has_pss.then_some(rollup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_rollup() {
        let rollup = SmapsRollup::parse(include_str!("../../fixtures/smaps_rollup"));
        assert_eq!(
            rollup,
            Some(SmapsRollup {
                rss_kib: 4136,
                pss_kib: 255,
                pss_anon_kib: 116,
                pss_file_kib: 139,
                pss_shmem_kib: 0,
                swap_kib: 0,
                swap_pss_kib: 0,
            })
        );
        assert_eq!(SmapsRollup::parse("Rss: 12 kB\n"), None);
    }
}
