/// Position of `utime` among the fields after the command name, which starts at `state`.
const UTIME_AFTER_COMMAND: usize = 11;

/// `utime + stime` of a `/proc/<pid>/stat` line, in clock ticks. The command
/// name may hold spaces and parentheses, so fields count from its last `)`.
pub fn cpu_ticks(stat: &str) -> Option<u64> {
    let (_, fields) = stat.rsplit_once(')')?;
    let mut fields = fields.split_whitespace().skip(UTIME_AFTER_COMMAND);
    let user: u64 = fields.next()?.parse().ok()?;
    let system: u64 = fields.next()?.parse().ok()?;
    Some(user + system)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cpu_ticks_past_odd_command_names() {
        assert_eq!(cpu_ticks(include_str!("../../fixtures/stat")), Some(1290));
        assert_eq!(cpu_ticks("812 (composer"), None);
    }
}
