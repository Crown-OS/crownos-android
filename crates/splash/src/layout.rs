//! Where the icon sits in the splash window.

use crate::LogicalSize;

/// Side of the icon in logical pixels, when the window has room for it.
pub const ICON_SIZE: u32 = 96;

/// A square in logical window coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconPlace {
    pub x: i32,
    pub y: i32,
    pub size: u32,
}

/// The icon centred in `window`, never more than half of its shorter side.
pub fn icon_place(window: LogicalSize) -> IconPlace {
    let size = ICON_SIZE.min(window.width.min(window.height) / 2).max(1);
    let offset = |length: u32| i32::try_from(length.saturating_sub(size) / 2).unwrap_or(0);
    IconPlace {
        x: offset(window.width),
        y: offset(window.height),
        size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_centred() {
        let place = icon_place(LogicalSize {
            width: 1280,
            height: 800,
        });
        assert_eq!(
            place,
            IconPlace {
                x: 592,
                y: 352,
                size: ICON_SIZE
            }
        );
    }

    #[test]
    fn a_tiny_window_shrinks_the_icon() {
        let place = icon_place(LogicalSize {
            width: 300,
            height: 120,
        });
        assert_eq!(
            place,
            IconPlace {
                x: 120,
                y: 30,
                size: 60
            }
        );
    }
}
