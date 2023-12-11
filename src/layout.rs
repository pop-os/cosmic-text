// SPDX-License-Identifier: MIT OR Apache-2.0

use core::fmt::Display;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{CacheKey, Color};

/// A laid out glyph
#[derive(Clone, Debug)]
pub struct LayoutGlyph {
    /// Start index of cluster in original line
    pub start: usize,
    /// End index of cluster in original line
    pub end: usize,
    /// Font size of the glyph
    pub font_size: f32,
    /// Line height
    pub line_height: f32,
    /// Font id of the glyph
    pub font_id: fontdb::ID,
    /// Font id of the glyph
    pub glyph_id: u16,
    /// X offset of hitbox
    pub x: f32,
    /// Y offset of hitbox
    pub y: f32,
    /// Width of hitbox
    pub w: f32,
    /// Unicode BiDi embedding level, character is left-to-right if `level` is divisible by 2
    pub level: unicode_bidi::Level,
    /// X offset in line
    ///
    /// If you are dealing with physical coordinates, use [`Self::physical`] to obtain a
    /// [`PhysicalGlyph`] for rendering.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub x_offset: f32,
    /// Y offset in line
    ///
    /// If you are dealing with physical coordinates, use [`Self::physical`] to obtain a
    /// [`PhysicalGlyph`] for rendering.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub y_offset: f32,
    /// Optional color override
    pub color_opt: Option<Color>,
    /// Metadata from `Attrs`
    pub metadata: usize,
}

#[derive(Clone, Debug)]
pub struct PhysicalGlyph {
    /// Cache key, see [CacheKey]
    pub cache_key: CacheKey,
    /// Integer component of X offset in line
    pub x: i32,
    /// Integer component of Y offset in line
    pub y: i32,
}

impl LayoutGlyph {
    pub fn physical(&self, offset: (f32, f32), scale: f32) -> PhysicalGlyph {
        let x_offset = self.font_size * self.x_offset;
        let y_offset = self.font_size * self.y_offset;

        let (cache_key, x, y) = CacheKey::new(
            self.font_id,
            self.glyph_id,
            self.font_size * scale,
            (
                (self.x + x_offset) * scale + offset.0,
                libm::truncf((self.y - y_offset) * scale + offset.1), // Hinting in Y axis
            ),
        );

        PhysicalGlyph { cache_key, x, y }
    }
}

/// A line of laid out glyphs
#[derive(Clone, Debug)]
pub struct LayoutLine {
    /// Width of the line
    pub width: f32,
    /// Maximum ascent of the glyphs in line
    pub max_ascent: f32,
    /// Maximum descent of the glyphs in line
    pub max_descent: f32,
    /// Glyphs in line
    pub glyphs: Vec<LayoutGlyph>,
    /// Height of the line, calculated from the glyphs at creation
    height: f32,
}

impl LayoutLine {
    /// Creates a new layoutline and  calculates the line height from the largest characters in the line,
    // if the line has no characters, the height will be zero
    pub fn new(width: f32, max_ascent: f32, max_descent: f32, glyphs: Vec<LayoutGlyph>) -> Self {
        // Calculates the line height from the largest characters in the line, if the line has no characters
        // well the line height is 0
        let height = glyphs
            .iter()
            .map(|g| g.line_height)
            .reduce(f32::max)
            .unwrap_or(0.0);
        Self {
            width,
            max_ascent,
            max_descent,
            glyphs,
            height,
        }
    }
    /// generates an empty layoutline with only a height
    pub fn empty_with_height(height: f32) -> Self {
        Self {
            height,
            width: 0.,
            max_ascent: 0.,
            max_descent: 0.,
            glyphs: Vec::new(),
        }
    }

    pub fn line_height(&self) -> f32 {
        self.height
    }
    /// Calculates the line height from the lines last characters line height
    pub fn last_char_line_height(&self) -> Option<f32> {
        self.glyphs.iter().last().map(|g| g.line_height)
    }
}

/// Wrapping mode
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Wrap {
    /// No wrapping
    None,
    /// Wraps at a glyph level
    Glyph,
    /// Word Wrapping
    Word,
}

impl Display for Wrap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "No Wrap"),
            Self::Word => write!(f, "Word Wrap"),
            Self::Glyph => write!(f, "Character"),
        }
    }
}

/// Align or justify
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Align {
    Left,
    Right,
    Center,
    Justified,
    End,
}

impl Display for Align {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Center => write!(f, "Center"),
            Self::Justified => write!(f, "Justified"),
            Self::End => write!(f, "End"),
        }
    }
}
