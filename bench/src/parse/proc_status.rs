use super::pairs::value_of;

/// Voluntary plus involuntary context switches of a `/proc/<pid>/task/<tid>/status`.
pub fn context_switches(status: &str) -> Option<u64> {
    Some(
        value_of(status, "voluntary_ctxt_switches")?
            + value_of(status, "nonvoluntary_ctxt_switches")?,
    )
}

pub fn parent_pid(status: &str) -> Option<u32> {
    value_of(status, "PPid").and_then(|pid| u32::try_from(pid).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = include_str!("../../fixtures/status");

    #[test]
    fn reads_switches_and_parent() {
        assert_eq!(context_switches(STATUS), Some(4058));
        assert_eq!(parent_pid(STATUS), Some(2190));
        assert_eq!(context_switches("Name:\tsh\n"), None);
    }
}
