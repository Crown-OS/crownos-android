//! Decoding an app icon and drawing it at one pixel size.

use std::path::Path;

use tiny_skia::{
    Color, FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Transform,
};

/// Opacity of the icon once the launch failed.
const FAILED_OPACITY: f32 = 0.35;
/// Grey of the disc drawn for an app without an icon.
const PLACEHOLDER_GREY: u8 = 0x9e;

/// Decodes the PNG at `path`, or `None` when it is missing or unreadable.
pub fn load(path: &Path) -> Option<Pixmap> {
    let bytes = std::fs::read(path).ok()?;
    Pixmap::decode_png(&bytes)
        .inspect_err(|error| log::warn!("could not decode {}: {error}", path.display()))
        .ok()
}

/// Whether the drawn icon stands for a failed launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Normal,
    Failed,
}

/// `source` scaled into a `side`×`side` square, or a grey disc without one.
pub fn draw(source: Option<&Pixmap>, side: u32, look: Look) -> Option<Pixmap> {
    let mut target = Pixmap::new(side, side)?;
    let opacity = match look {
        Look::Normal => 1.0,
        Look::Failed => FAILED_OPACITY,
    };
    let side = side as f32;
    match source {
        Some(source) => {
            let paint = PixmapPaint {
                opacity,
                quality: FilterQuality::Bicubic,
                ..PixmapPaint::default()
            };
            let transform =
                Transform::from_scale(side / source.width() as f32, side / source.height() as f32);
            target.draw_pixmap(0, 0, source.as_ref(), &paint, transform, None);
        }
        None => {
            let radius = side / 2.0;
            let disc = PathBuilder::from_circle(radius, radius, radius)?;
            let mut paint = Paint::default();
            let mut color = Color::from_rgba8(
                PLACEHOLDER_GREY,
                PLACEHOLDER_GREY,
                PLACEHOLDER_GREY,
                u8::MAX,
            );
            color.apply_opacity(opacity);
            paint.set_color(color);
            paint.anti_alias = true;
            target.fill_path(
                &disc,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_PIXMAP: &str = "pixmap could not be created";

    fn filled(side: u32, color: Color) -> Result<Pixmap, &'static str> {
        let mut pixmap = Pixmap::new(side, side).ok_or(NO_PIXMAP)?;
        pixmap.fill(color);
        Ok(pixmap)
    }

    fn alpha_at(pixmap: &Pixmap, x: u32, y: u32) -> Result<u8, &'static str> {
        Ok(pixmap.pixel(x, y).ok_or("pixel out of range")?.alpha())
    }

    #[test]
    fn scales_the_icon_to_the_requested_side() -> Result<(), &'static str> {
        let source = filled(4, Color::from_rgba8(200, 0, 0, 255))?;
        let drawn = draw(Some(&source), 12, Look::Normal).ok_or(NO_PIXMAP)?;
        assert_eq!(drawn.width(), 12);
        let center = drawn.pixel(6, 6).ok_or("pixel out of range")?;
        assert_eq!((center.red(), center.alpha()), (200, 255));
        Ok(())
    }

    #[test]
    fn a_failed_launch_fades_the_icon() -> Result<(), &'static str> {
        let source = filled(2, Color::WHITE)?;
        let faded = draw(Some(&source), 8, Look::Failed).ok_or(NO_PIXMAP)?;
        assert!(alpha_at(&faded, 4, 4)? < 128);
        Ok(())
    }

    #[test]
    fn an_app_without_icon_gets_a_disc() -> Result<(), &'static str> {
        let disc = draw(None, 16, Look::Normal).ok_or(NO_PIXMAP)?;
        assert_eq!(alpha_at(&disc, 0, 0)?, 0);
        assert_eq!(alpha_at(&disc, 8, 8)?, 255);
        Ok(())
    }
}
