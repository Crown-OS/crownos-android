//! The splash background: the icon's own hue, softened for the desktop's tone.

/// Pixels fainter than this do not count towards the icon's colour.
const MIN_ALPHA: u8 = 32;
/// Lightness of the background in a light and a dark desktop.
const LIGHT_LIGHTNESS: f32 = 0.92;
const DARK_LIGHTNESS: f32 = 0.16;
/// The background never gets more saturated than this.
const MAX_SATURATION: f32 = 0.45;
const NEUTRAL_LIGHT: Rgb = Rgb::new(0xee, 0xee, 0xee);
const NEUTRAL_DARK: Rgb = Rgb::new(0x24, 0x24, 0x24);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Whether the desktop is light or dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Light,
    Dark,
}

/// The background for an icon given as premultiplied RGBA bytes: the
/// alpha-weighted average hue of its visible pixels, at the tone's lightness.
/// A blank icon gets a neutral grey.
pub fn background(premultiplied_rgba: &[u8], tone: Tone) -> Rgb {
    let mut sums = [0u64; 4];
    for pixel in premultiplied_rgba.chunks_exact(4) {
        if pixel[3] < MIN_ALPHA {
            continue;
        }
        for (sum, channel) in sums.iter_mut().zip(pixel) {
            *sum += u64::from(*channel);
        }
    }
    let alpha = sums[3];
    if alpha == 0 {
        return match tone {
            Tone::Light => NEUTRAL_LIGHT,
            Tone::Dark => NEUTRAL_DARK,
        };
    }
    let channel = |sum: u64| sum as f32 / alpha as f32;
    let (hue, saturation, _) = to_hsl(channel(sums[0]), channel(sums[1]), channel(sums[2]));
    let lightness = match tone {
        Tone::Light => LIGHT_LIGHTNESS,
        Tone::Dark => DARK_LIGHTNESS,
    };
    from_hsl(hue, saturation.min(MAX_SATURATION), lightness)
}

/// Hue in turns, saturation and lightness, from channels in `0..=1`.
fn to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let chroma = max - min;
    if chroma <= f32::EPSILON {
        return (0.0, 0.0, lightness);
    }
    let saturation = chroma / (1.0 - (2.0 * lightness - 1.0).abs());
    let sector = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    (sector / 6.0, saturation.min(1.0), lightness)
}

fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Rgb {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue * 6.0;
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let offset = lightness - chroma / 2.0;
    let byte = |value: f32| ((value + offset) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb::new(byte(r), byte(g), byte(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(rgba: [u8; 4], count: usize) -> Vec<u8> {
        rgba.repeat(count)
    }

    #[test]
    fn a_red_icon_gets_a_light_red_background() {
        let background = background(&icon([220, 30, 30, 255], 16), Tone::Light);
        assert!(background.r > background.g && background.r > background.b);
        assert!(
            background.g > 200,
            "light enough to read as a tint: {background:?}"
        );
        assert_eq!(background.g, background.b);
    }

    #[test]
    fn a_dark_desktop_gets_a_dark_tint_of_the_same_hue() {
        let background = background(&icon([30, 30, 220, 255], 16), Tone::Dark);
        assert!(background.b > background.r && background.b > background.g);
        assert!(background.b < 80);
    }

    #[test]
    fn transparent_pixels_do_not_count() {
        let mut pixels = icon([0, 200, 0, 255], 4);
        pixels.extend(icon([200, 0, 0, 10], 400));
        let background = background(&pixels, Tone::Light);
        assert!(background.g > background.r);
    }

    #[test]
    fn a_blank_icon_gets_a_neutral_grey() {
        assert_eq!(
            background(&icon([0, 0, 0, 0], 8), Tone::Light),
            NEUTRAL_LIGHT
        );
        assert_eq!(background(&[], Tone::Dark), NEUTRAL_DARK);
    }

    #[test]
    fn a_grey_icon_stays_grey() {
        let background = background(&icon([128, 128, 128, 255], 4), Tone::Light);
        assert_eq!(background.r, background.g);
        assert_eq!(background.g, background.b);
    }
}
