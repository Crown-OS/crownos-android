use std::path::Path;

use crate::VenusError;
use crate::util::kernel_files;

/// The kernel driver name of NVIDIA GPUs, open and closed module alike.
pub const DRIVER: &str = "nvidia";

const VERSION_FILE: &str = "driver/nvidia/version";
const NVRM_PREFIX: &str = "NVRM version:";
const OPEN_MODULE_MARKER: &str = "Open Kernel Module";
const MODESET_PARAMETER: &str = "module/nvidia_drm/parameters/modeset";
const MODESET_ENABLED: &str = "Y";

/// The version of a loaded open NVIDIA kernel module, for example `615.78.08`.
#[derive(Debug, PartialEq, Eq)]
pub struct OpenModuleVersion {
    text: String,
    major: u32,
}

impl OpenModuleVersion {
    /// Reads `driver/nvidia/version` under `proc_root`. `None` when no NVIDIA
    /// module is loaded or the loaded one is the closed module, which cannot
    /// export dma-bufs.
    pub fn read(proc_root: &Path) -> Result<Option<Self>, VenusError> {
        let version_file = kernel_files::read_value(&proc_root.join(VERSION_FILE))?;
        Ok(version_file.as_deref().and_then(Self::parse))
    }

    pub fn major(&self) -> u32 {
        self.major
    }

    pub fn into_string(self) -> String {
        self.text
    }

    fn parse(version_file: &str) -> Option<Self> {
        let nvrm = version_file
            .lines()
            .find_map(|line| line.strip_prefix(NVRM_PREFIX))?;
        if !nvrm.contains(OPEN_MODULE_MARKER) {
            return None;
        }
        let text = nvrm
            .split_whitespace()
            .find(|token| is_dotted_number(token))?;
        let major = text.split('.').next()?.parse().ok()?;
        Some(Self {
            text: text.to_owned(),
            major,
        })
    }
}

/// Whether `nvidia_drm` runs with `modeset=1`, without which the driver has
/// no `VK_EXT_external_memory_dma_buf`.
pub fn modeset_enabled(sys: &Path) -> Result<bool, VenusError> {
    let modeset = kernel_files::read_value(&sys.join(MODESET_PARAMETER))?;
    Ok(modeset.as_deref() == Some(MODESET_ENABLED))
}

fn is_dotted_number(token: &str) -> bool {
    token.contains('.')
        && token
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPEN_615: &str = "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  615.78.08  Release Build  (root@)  \nGCC version:  gcc version 16.2.1 20260810 (GCC) \n";
    const CLOSED_615: &str = "NVRM version: NVIDIA UNIX x86_64 Kernel Module  615.78.08  Release Build  (builder@host)  Tue Oct  6 12:00:00 UTC 2026\nGCC version:  gcc version 16.2.1 20260810 (GCC) \n";

    #[test]
    fn the_open_module_version_is_parsed() {
        let version = OpenModuleVersion::parse(OPEN_615);
        assert_eq!(
            version,
            Some(OpenModuleVersion {
                text: "615.78.08".into(),
                major: 615,
            })
        );
    }

    #[test]
    fn the_closed_module_has_no_open_version() {
        assert_eq!(OpenModuleVersion::parse(CLOSED_615), None);
    }

    #[test]
    fn a_file_without_an_nvrm_line_has_no_version() {
        assert_eq!(OpenModuleVersion::parse("GCC version: 16.2.1\n"), None);
    }

    #[test]
    fn version_tokens_are_dotted_numbers() {
        assert!(is_dotted_number("595.71.05"));
        assert!(is_dotted_number("610.10"));
        assert!(!is_dotted_number("x86_64"));
        assert!(!is_dotted_number("12:00:00"));
        assert!(!is_dotted_number("2026"));
        assert!(!is_dotted_number("595."));
    }
}
