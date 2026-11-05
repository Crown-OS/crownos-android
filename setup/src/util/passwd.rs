use std::ops::RangeInclusive;

/// The range `useradd` gives regular users (`login.defs`).
const USER_UIDS: RangeInclusive<u32> = 1000..=60000;

/// Home directories of the regular users in `/etc/passwd` text.
pub fn user_homes(passwd: &str) -> impl Iterator<Item = &str> {
    passwd.lines().filter_map(|line| {
        let mut fields = line.split(':');
        let uid = fields.nth(2)?.parse::<u32>().ok()?;
        let home = fields.nth(2)?;
        (USER_UIDS.contains(&uid) && home.starts_with('/')).then_some(home)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_regular_users_with_absolute_homes_count() {
        let passwd = "root:x:0:0::/root:/usr/bin/bash
nobody:x:65534:65534:Kernel Overflow User:/:/usr/bin/nologin
alice:x:1000:1000:Alice:/home/alice:/usr/bin/fish
bob:x:1001:1001::/home/bob:/usr/bin/bash
odd:x:1002:1002::relative:/usr/bin/bash
broken line
";
        assert_eq!(
            user_homes(passwd).collect::<Vec<_>>(),
            ["/home/alice", "/home/bob"]
        );
    }
}
