use crate::key::Rgb;

/// A colour in QMK's HSV space: every channel is `0..=255`, hue wraps
/// (0 red, 85 green, 171 blue).
///
/// The stock firmware only takes colour as HSV, so the global lighting path
/// converts [`Rgb`] with [`Hsv::from`]. See ADR 0002 for why the conversion
/// lives on the host and how `v` maps to brightness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hsv {
    /// Hue, `0..=255` around the colour wheel.
    pub h: u8,
    /// Saturation.
    pub s: u8,
    /// Value. On the global path this is sent as RGB matrix brightness.
    pub v: u8,
}

impl From<Rgb> for Hsv {
    /// Integer RGB to HSV on QMK's 0..=255 scale, the inverse of QMK's
    /// `hsv_to_rgb` up to rounding (one sixth of the wheel is 43 steps).
    #[allow(clippy::many_single_char_names)]
    fn from(Rgb { r, g, b }: Rgb) -> Self {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = i32::from(max - min);
        if delta == 0 {
            return Self { h: 0, s: 0, v: max };
        }
        let s = u8::try_from(255 * delta / i32::from(max)).unwrap_or(u8::MAX);
        let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
        let max = i32::from(max);
        let h = if max == r {
            43 * (g - b) / delta
        } else if max == g {
            85 + 43 * (b - r) / delta
        } else {
            171 + 43 * (r - g) / delta
        };
        let h = u8::try_from(h.rem_euclid(256)).unwrap_or(0);
        Self {
            h,
            s,
            v: u8::try_from(max).unwrap_or(u8::MAX),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let Hsv { h, s, v } = Rgb { r, g, b }.into();
        (h, s, v)
    }

    #[test]
    fn primaries_land_on_qmk_hues() {
        assert_eq!(hsv(255, 0, 0), (0, 255, 255));
        assert_eq!(hsv(0, 255, 0), (85, 255, 255));
        assert_eq!(hsv(0, 0, 255), (171, 255, 255));
        assert_eq!(hsv(255, 255, 0), (43, 255, 255));
        // QMK HSV_MAGENTA is 213.
        assert_eq!(hsv(255, 0, 255), (213, 255, 255));
    }

    #[test]
    fn greys_have_no_saturation() {
        assert_eq!(hsv(0, 0, 0), (0, 0, 0));
        assert_eq!(hsv(255, 255, 255), (0, 0, 255));
        assert_eq!(hsv(128, 128, 128), (0, 0, 128));
    }

    #[test]
    fn dim_colour_keeps_hue_and_moves_value() {
        assert_eq!(hsv(128, 0, 0), (0, 255, 128));
    }

    #[test]
    fn hue_wraps_below_red() {
        // Red with a little blue sits just under 256, not negative.
        assert_eq!(hsv(255, 0, 20).0, 253);
    }
}
