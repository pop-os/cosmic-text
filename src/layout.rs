// SPDX-License-Identifier: MIT OR Apache-2.0

use core::fmt::Display;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{CacheKey, Color};

/// A laid out glyph
#[derive(Debug)]
pub struct LayoutGlyph {
    /// Start index of cluster in original line
    pub start: usize,
    /// End index of cluster in original line
    pub end: usize,
    /// X offset of hitbox
    pub x: f32,
    /// width of hitbox
    pub w: f32,
    /// Unicode BiDi embedding level, character is left-to-right if `level` is divisible by 2
    pub level: unicode_bidi::Level,
    /// Cache key, see [CacheKey]
    pub cache_key: CacheKey,
    /// X offset in line
    ///
    /// If you are dealing with physical coordinates, you will want to use [`Self::x_int`]
    /// together with [`CacheKey::x_bin`] instead. This will ensure the best alignment of the
    /// rasterized glyphs with the pixel grid.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub x_offset: f32,
    /// Y offset in line
    ///
    /// If you are dealing with physical coordinates, you will want to use [`Self::y_int`]
    /// together with [`CacheKey::y_bin`] instead. This will ensure the best alignment of the
    /// rasterized glyphs with the pixel grid.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub y_offset: f32,
    /// Integer component of X offset in line
    pub x_int: i32,
    /// Integer component of Y offset in line
    pub y_int: i32,
    /// Optional color override
    pub color_opt: Option<Color>,
    /// Metadata from `Attrs`
    pub metadata: usize,
}

/// A line of laid out glyphs
pub struct LayoutLine {
    /// Width of the line
    pub w: f32,
    /// Glyphs in line
    pub glyphs: Vec<LayoutGlyph>,
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
