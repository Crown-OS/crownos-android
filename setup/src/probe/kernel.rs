use std::io;
use std::path::Path;

/// The only page size the pinned x86_64 and arm64 images are built for.
pub const SUPPORTED_PAGE_SIZE: u64 = 4096;

const AT_PAGESZ: u64 = 6;
const WORD: usize = size_of::<u64>();

/// Whether the kernel offers binderfs, the only binder Waydroid can use
/// without DKMS (`docs/decisions/s3-platform.md`).
pub fn has_binderfs(proc_root: &Path) -> io::Result<bool> {
    let filesystems = std::fs::read_to_string(proc_root.join("filesystems"))?;
    Ok(filesystems
        .lines()
        .any(|line| line.split_whitespace().last() == Some("binder")))
}

/// The kernel page size, from `AT_PAGESZ` in this process's auxiliary vector.
pub fn page_size(proc_root: &Path) -> io::Result<Option<u64>> {
    let auxv = std::fs::read(proc_root.join("self/auxv"))?;
    let word = |bytes: &[u8]| bytes.try_into().map(u64::from_ne_bytes).ok();
    Ok(auxv
        .chunks_exact(2 * WORD)
        .filter_map(|entry| Some((word(&entry[..WORD])?, word(&entry[WORD..])?)))
        .find_map(|(key, value)| (key == AT_PAGESZ).then_some(value)))
}

pub fn page_size_supported(page_size: Option<u64>) -> bool {
    page_size == Some(SUPPORTED_PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scratch::Scratch;

    fn auxv(entries: &[(u64, u64)]) -> Vec<u8> {
        entries
            .iter()
            .flat_map(|(key, value)| [key.to_ne_bytes(), value.to_ne_bytes()])
            .flatten()
            .collect()
    }

    fn tree(filesystems: &str, entries: &[(u64, u64)]) -> io::Result<Scratch> {
        let root = Scratch::new("kernel")?;
        std::fs::create_dir_all(root.join("self"))?;
        std::fs::write(root.join("filesystems"), filesystems)?;
        std::fs::write(root.join("self/auxv"), auxv(entries))?;
        Ok(root)
    }

    #[test]
    fn binderfs_is_found_in_the_filesystem_list() -> io::Result<()> {
        let root = tree(
            "nodev\tsysfs\nnodev\tbinder\n\text4\n",
            &[(AT_PAGESZ, 4096)],
        )?;
        assert!(has_binderfs(&root)?);
        std::fs::write(root.join("filesystems"), "nodev\tsysfs\n\text4\n")?;
        assert!(!has_binderfs(&root)?);
        Ok(())
    }

    #[test]
    fn four_kib_pages_are_supported_and_sixteen_kib_pages_are_not() -> io::Result<()> {
        for (size, supported) in [(4096, true), (16384, false)] {
            let root = tree("", &[(33, 0), (AT_PAGESZ, size), (0, 0)])?;
            assert_eq!(page_size(&root)?, Some(size));
            assert_eq!(page_size_supported(page_size(&root)?), supported);
        }
        Ok(())
    }

    #[test]
    fn the_real_kernel_reports_a_page_size() -> io::Result<()> {
        assert!(page_size(Path::new("/proc"))?.is_some());
        Ok(())
    }
}
