use super::pairs::value_of;

/// The CPU time a cgroup used, `usage_usec` of its `cpu.stat`, in microseconds.
pub fn usage_usec(cpu_stat: &str) -> Option<u64> {
    value_of(cpu_stat, "usage_usec")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cpu_usage() {
        assert_eq!(
            usage_usec(include_str!("../../fixtures/cpu.stat")),
            Some(259_809_065)
        );
        assert_eq!(usage_usec("nr_periods 0\n"), None);
    }
}
