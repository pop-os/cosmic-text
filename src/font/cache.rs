// SPDX-License-Identifier: MIT OR Apache-2.0

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey {
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    pub font_size: i32,
    pub x_bin: SubpixelBin,
    pub y_bin: SubpixelBin,
}

impl CacheKey {
    pub fn new(
        font_id: fontdb::ID,
        glyph_id: u16,
        font_size: i32,
        pos: (f32, f32),
    ) -> (Self, i32, i32) {
        let (x, x_bin) = SubpixelBin::new(pos.0);
        let (y, y_bin) = SubpixelBin::new(pos.1);
        (
            Self {
                font_id,
                glyph_id,
                font_size,
                x_bin,
                y_bin,
            },
            x,
            y,
        )
    }
}

pub type CacheItem = Option<swash::scale::image::Image>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SubpixelBin {
    Zero,
    One,
    Two,
    Three,
}

impl SubpixelBin {
    pub fn new(pos: f32) -> (i32, Self) {
        let trunc = pos.trunc() as i32;
        let fract = pos.fract();
        if pos.is_sign_negative() {
            if fract > -0.125 {
                (trunc, Self::Zero)
            } else if fract > -0.375 {
                (trunc - 1, Self::Three)
            } else if fract > -0.625 {
                (trunc - 1, Self::Two)
            } else if fract > -0.875 {
                (trunc - 1, Self::One)
            } else {
                (trunc - 1, Self::Zero)
            }
        } else {
            #[allow(clippy::collapsible_else_if)]
            if fract < 0.125 {
                (trunc, Self::Zero)
            } else if fract < 0.375 {
                (trunc, Self::One)
            } else if fract < 0.625 {
                (trunc, Self::Two)
            } else if fract < 0.875 {
                (trunc, Self::Three)
            } else {
                (trunc + 1, Self::Zero)
            }
        }
    }

    pub fn as_float(&self) -> f32 {
        match self {
            Self::Zero => 0.0,
            Self::One => 0.25,
            Self::Two => 0.5,
            Self::Three => 0.75,
        }
    }
}

#[test]
fn test_subpixel_bins() {
    // POSITIVE TESTS

    // Maps to 0.0
    assert_eq!(SubpixelBin::new(0.0), (0, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(0.124), (0, SubpixelBin::Zero));

    // Maps to 0.25
    assert_eq!(SubpixelBin::new(0.125), (0, SubpixelBin::One));
    assert_eq!(SubpixelBin::new(0.25), (0, SubpixelBin::One));
    assert_eq!(SubpixelBin::new(0.374), (0, SubpixelBin::One));

    // Maps to 0.5
    assert_eq!(SubpixelBin::new(0.375), (0, SubpixelBin::Two));
    assert_eq!(SubpixelBin::new(0.5), (0, SubpixelBin::Two));
    assert_eq!(SubpixelBin::new(0.624), (0, SubpixelBin::Two));

    // Maps to 0.75
    assert_eq!(SubpixelBin::new(0.625), (0, SubpixelBin::Three));
    assert_eq!(SubpixelBin::new(0.75), (0, SubpixelBin::Three));
    assert_eq!(SubpixelBin::new(0.874), (0, SubpixelBin::Three));

    // Maps to 1.0
    assert_eq!(SubpixelBin::new(0.875), (1, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(0.999), (1, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(1.0), (1, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(1.124), (1, SubpixelBin::Zero));

    // NEGATIVE TESTS

    // Maps to 0.0
    assert_eq!(SubpixelBin::new(-0.0), (0, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(-0.124), (0, SubpixelBin::Zero));

    // Maps to 0.25
    assert_eq!(SubpixelBin::new(-0.125), (-1, SubpixelBin::Three));
    assert_eq!(SubpixelBin::new(-0.25), (-1, SubpixelBin::Three));
    assert_eq!(SubpixelBin::new(-0.374), (-1, SubpixelBin::Three));

    // Maps to 0.5
    assert_eq!(SubpixelBin::new(-0.375), (-1, SubpixelBin::Two));
    assert_eq!(SubpixelBin::new(-0.5), (-1, SubpixelBin::Two));
    assert_eq!(SubpixelBin::new(-0.624), (-1, SubpixelBin::Two));

    // Maps to 0.75
    assert_eq!(SubpixelBin::new(-0.625), (-1, SubpixelBin::One));
    assert_eq!(SubpixelBin::new(-0.75), (-1, SubpixelBin::One));
    assert_eq!(SubpixelBin::new(-0.874), (-1, SubpixelBin::One));

    // Maps to 1.0
    assert_eq!(SubpixelBin::new(-0.875), (-1, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(-0.999), (-1, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(-1.0), (-1, SubpixelBin::Zero));
    assert_eq!(SubpixelBin::new(-1.124), (-1, SubpixelBin::Zero));
}
