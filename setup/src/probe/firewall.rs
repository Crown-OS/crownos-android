use std::path::Path;

const CONFLICTS: [(&str, &str); 3] = [
    ("firewalld", "usr/bin/firewalld"),
    ("ufw", "usr/bin/ufw"),
    ("docker", "usr/bin/dockerd"),
];

/// Installed firewalls whose rules can break Waydroid's NAT (Waydroid #2458).
pub fn conflicts(root: &Path) -> Vec<&'static str> {
    CONFLICTS
        .iter()
        .filter(|(_, program)| root.join(program).exists())
        .map(|(name, _)| *name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scratch::Scratch;

    #[test]
    fn installed_firewalls_are_named() -> std::io::Result<()> {
        let root = Scratch::new("firewall")?;
        std::fs::create_dir_all(root.join("usr/bin"))?;
        assert!(conflicts(&root).is_empty());
        std::fs::write(root.join("usr/bin/ufw"), "")?;
        std::fs::write(root.join("usr/bin/dockerd"), "")?;
        assert_eq!(conflicts(&root), ["ufw", "docker"]);
        Ok(())
    }
}
