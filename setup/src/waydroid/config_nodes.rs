use std::io;
use std::path::{Path, PathBuf};

use venus::Support;

use crate::binder::NODES;
use crate::util::atomic;
use crate::vgem;

const MOUNT_ENTRY: &str = "lxc.mount.entry";

/// The entry that binds canonical binder node `name` into the container, in
/// the form Waydroid itself writes.
pub fn binder_line(name: &str) -> String {
    format!("{MOUNT_ENTRY} = /dev/{name} dev/{name} none bind,create=file,optional 0 0")
}

/// The lines CrownOS keeps in `config_nodes` beside Waydroid's own.
#[derive(Clone, Copy, Debug, Default)]
pub struct Managed<'a> {
    /// The Venus tier's NVIDIA render node.
    pub venus: Option<&'a str>,
    /// vgem's render node, for the hwcomposer's fences.
    pub vgem: Option<&'a str>,
}

/// Rewrites Waydroid's `config_nodes` so every binder entry binds the
/// canonical node, whatever names the last `init` or `upgrade` saw. Each
/// wanted [`Managed`] line is present once. Lines Venus wrote for another
/// render node are gone, because a stale non-optional bind stops LXC, and so
/// are vgem lines that no longer match the loaded module.
pub fn rewrite(text: &str, managed: Managed<'_>) -> String {
    let mut lines = Vec::new();
    let mut binder_written = [false; NODES.len()];
    for line in text.lines() {
        let Some((source, destination)) = parse_entry(line) else {
            lines.push(line.to_owned());
            continue;
        };
        if let Some(index) = binder_index(destination) {
            if !binder_written[index] {
                lines.push(binder_line(NODES[index]));
                binder_written[index] = true;
            }
        } else if keeps(line, source, managed) {
            lines.push(line.to_owned());
        }
    }
    for (name, written) in NODES.iter().zip(binder_written) {
        if !written {
            lines.push(binder_line(name));
        }
    }
    for wanted in [managed.venus, managed.vgem].into_iter().flatten() {
        if !lines.iter().any(|line| line.trim() == wanted) {
            lines.push(wanted.to_owned());
        }
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Applies [`rewrite`] to the file at `path` and reports whether it changed.
/// A missing file means Waydroid is not initialised yet; `init` writes it.
pub fn update(path: &Path, managed: Managed<'_>) -> io::Result<bool> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let rewritten = rewrite(&text, managed);
    if rewritten == text {
        return Ok(false);
    }
    atomic::write(path, rewritten.as_bytes())?;
    Ok(true)
}

fn keeps(line: &str, source: &str, managed: Managed<'_>) -> bool {
    if vgem::is_mount_line(line) {
        return managed.vgem == Some(line.trim());
    }
    managed
        .venus
        .is_none_or(|wanted| line.trim() == wanted || !is_venus_line(line, source))
}

fn parse_entry(line: &str) -> Option<(&str, &str)> {
    let mut fields = line
        .trim()
        .strip_prefix(MOUNT_ENTRY)?
        .trim_start()
        .strip_prefix('=')?
        .split_whitespace();
    Some((fields.next()?, fields.next()?))
}

fn binder_index(destination: &str) -> Option<usize> {
    let name = destination.strip_prefix("dev/")?;
    NODES.iter().position(|node| *node == name)
}

fn is_venus_line(line: &str, source: &str) -> bool {
    let support = Support {
        render_node: PathBuf::from(source),
        driver_version: String::new(),
    };
    venus::container_mount_line(&support) == line.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFTER_INIT_ON_ZEN: &str = "lxc.mount.entry = tmpfs dev tmpfs nosuid 0 0
lxc.mount.entry = /dev/zero dev/zero none bind,create=file,optional 0 0
lxc.mount.entry = /dev/anbox-binder dev/binder none bind,create=file,optional 0 0
lxc.mount.entry = /dev/anbox-vndbinder dev/vndbinder none bind,create=file,optional 0 0
lxc.mount.entry = /dev/anbox-hwbinder dev/hwbinder none bind,create=file,optional 0 0
lxc.mount.entry = /dev/uhid dev/uhid none bind,create=file,optional 0 0
";

    fn venus(line: &str) -> Managed<'_> {
        Managed {
            venus: Some(line),
            vgem: None,
        }
    }

    fn vgem(line: &str) -> Managed<'_> {
        Managed {
            venus: None,
            vgem: Some(line),
        }
    }

    fn venus_line(node: &str) -> String {
        venus::container_mount_line(&Support {
            render_node: node.into(),
            driver_version: "615.78.08".into(),
        })
    }

    #[test]
    fn binder_entries_are_pinned_to_the_canonical_nodes_in_place() {
        let rewritten = rewrite(AFTER_INIT_ON_ZEN, Managed::default());
        let lines: Vec<_> = rewritten.lines().collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[2], binder_line("binder"));
        assert_eq!(lines[4], binder_line("hwbinder"));
        assert!(!rewritten.contains("anbox"));
        assert_eq!(rewrite(&rewritten, Managed::default()), rewritten);
    }

    #[test]
    fn missing_binder_entries_are_added() {
        let rewritten = rewrite(
            "lxc.mount.entry = /dev/zero dev/zero none bind,create=file,optional 0 0\n",
            Managed::default(),
        );
        for node in NODES {
            assert!(rewritten.contains(&binder_line(node)));
        }
    }

    #[test]
    fn the_venus_line_is_added_once() {
        let line = venus_line("/dev/dri/renderD129");
        let once = rewrite(AFTER_INIT_ON_ZEN, venus(&line));
        assert_eq!(once.lines().filter(|found| *found == line).count(), 1);
        assert_eq!(rewrite(&once, venus(&line)), once);
    }

    #[test]
    fn a_venus_line_for_a_renumbered_node_is_replaced() {
        let stale = format!("{AFTER_INIT_ON_ZEN}{}\n", venus_line("/dev/dri/renderD128"));
        let line = venus_line("/dev/dri/renderD129");
        let rewritten = rewrite(&stale, venus(&line));
        assert!(!rewritten.contains("renderD128"));
        assert!(rewritten.contains(&line));
    }

    #[test]
    fn waydroid_render_entries_stay_when_venus_is_off() {
        let native = format!(
            "{AFTER_INIT_ON_ZEN}lxc.mount.entry = /dev/dri/renderD128 dev/dri/renderD128 none bind,create=file,optional 0 0\n"
        );
        assert!(rewrite(&native, Managed::default()).contains("renderD128"));
    }

    #[test]
    fn a_missing_file_is_left_for_waydroid_init() -> io::Result<()> {
        assert!(!update(
            Path::new("/nonexistent/config_nodes"),
            Managed::default()
        )?);
        Ok(())
    }

    #[test]
    fn the_vgem_line_is_added_once_and_follows_a_renumbered_node() {
        let line = vgem::mount_line(Path::new("/dev/dri/renderD130"));
        let once = rewrite(AFTER_INIT_ON_ZEN, vgem(&line));
        assert_eq!(once.lines().filter(|found| *found == line).count(), 1);
        assert_eq!(rewrite(&once, vgem(&line)), once);

        let moved = vgem::mount_line(Path::new("/dev/dri/renderD131"));
        let rewritten = rewrite(&once, vgem(&moved));
        assert!(!rewritten.contains("renderD130"));
        assert!(rewritten.contains(&moved));
    }

    #[test]
    fn a_vgem_line_goes_when_vgem_is_gone_and_waydroids_gpu_line_stays() {
        let gpu = "lxc.mount.entry = /dev/dri/renderD128 dev/dri/renderD128 none bind,create=file,optional 0 0";
        let with_vgem = format!(
            "{AFTER_INIT_ON_ZEN}{gpu}\n{}\n",
            vgem::mount_line(Path::new("/dev/dri/renderD130"))
        );
        let rewritten = rewrite(&with_vgem, Managed::default());
        assert!(!rewritten.contains("renderD130"));
        assert!(rewritten.contains(gpu));
    }
}
