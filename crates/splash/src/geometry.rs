//! Sizes and scales, and the rule for how big a launched app's window is.

/// Fractional scales count in 120ths (`wp_fractional_scale_v1`).
const SCALE_DENOMINATOR: u32 = 120;
/// Share of the primary output a launched window takes, in percent.
const OUTPUT_SHARE_PERCENT: u32 = 60;
/// Largest launch size, in logical pixels.
pub const MAX_LAUNCH: LogicalSize = LogicalSize {
    width: 1280,
    height: 800,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

/// An output scale in 120ths: 120 is 1×, 180 is 1.5×.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale(u32);

impl Scale {
    pub const ONE: Self = Self(SCALE_DENOMINATOR);

    pub fn from_120ths(value: u32) -> Self {
        Self(value.max(1))
    }

    pub fn from_integer(factor: i32) -> Self {
        Self(u32::try_from(factor).unwrap_or(1).max(1) * SCALE_DENOMINATOR)
    }

    /// `logical` pixels at this scale, rounded to the nearest pixel.
    pub fn physical(self, logical: u32) -> u32 {
        let scaled = u64::from(logical) * u64::from(self.0) + u64::from(SCALE_DENOMINATOR / 2);
        u32::try_from(scaled / u64::from(SCALE_DENOMINATOR)).unwrap_or(u32::MAX)
    }
}

impl LogicalSize {
    pub fn to_physical(self, scale: Scale) -> PhysicalSize {
        PhysicalSize {
            width: scale.physical(self.width),
            height: scale.physical(self.height),
        }
    }
}

/// The logical size of a launched app's window: 60% of the primary output,
/// clamped to [`MAX_LAUNCH`]. Without a known output it is [`MAX_LAUNCH`].
pub fn launch_size(output: Option<LogicalSize>) -> LogicalSize {
    let Some(output) = output else {
        return MAX_LAUNCH;
    };
    let share = |length: u32| length * OUTPUT_SHARE_PERCENT / 100;
    LogicalSize {
        width: share(output.width).clamp(1, MAX_LAUNCH.width),
        height: share(output.height).clamp(1, MAX_LAUNCH.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn size(width: u32, height: u32) -> LogicalSize {
        LogicalSize { width, height }
    }

    #[test]
    fn a_small_output_gets_sixty_percent_of_itself() {
        assert_eq!(launch_size(Some(size(1366, 768))), size(819, 460));
    }

    #[test]
    fn a_large_output_is_clamped_to_1280_by_800() {
        assert_eq!(launch_size(Some(size(3840, 2160))), MAX_LAUNCH);
        assert_eq!(launch_size(Some(size(1920, 1080))), size(1152, 648));
        assert_eq!(launch_size(None), MAX_LAUNCH);
    }

    #[test]
    fn android_pixels_are_logical_pixels_times_the_scale() {
        assert_eq!(
            MAX_LAUNCH.to_physical(Scale::from_120ths(180)),
            PhysicalSize {
                width: 1920,
                height: 1200
            }
        );
        assert_eq!(
            size(819, 460).to_physical(Scale::from_120ths(150)).width,
            1024
        );
        assert_eq!(Scale::from_integer(2), Scale::from_120ths(240));
        assert_eq!(Scale::ONE.physical(96), 96);
    }
}
