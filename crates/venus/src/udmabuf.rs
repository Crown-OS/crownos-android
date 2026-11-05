use std::path::Path;

use crate::VenusError;
use crate::util::kernel_files;

const MISC_DEVICE: &str = "class/misc/udmabuf";

/// Whether the kernel registered the udmabuf misc device, for which devtmpfs
/// creates `/dev/udmabuf`. The render server backs CPU-mappable gralloc
/// buffers with it.
pub fn is_registered(sys: &Path) -> Result<bool, VenusError> {
    kernel_files::exists(&sys.join(MISC_DEVICE))
}
