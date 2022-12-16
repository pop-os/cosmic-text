// SPDX-License-Identifier: MIT OR Apache-2.0

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
    /// True if the character is from an RTL script
    pub rtl: bool,
    /// Cache key, see [CacheKey]
    pub cache_key: CacheKey,
    /// X offset in line
    ///
    /// Unless you are not dealing with physical coordinates, you will want to use [`Self::x_int`]
    /// together with [`CacheKey::x_bin`] instead. This will ensure the best alignment of the
    /// rasterized glyphs with the pixel grid.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub x_offset: f32,
    /// Y offset in line
    ///
    /// Unless you are not dealing with physical coordinates, you will want to use [`Self::y_int`]
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
