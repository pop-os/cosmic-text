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
    /// Integer component of X offset in line
    pub x_int: i32,
    /// Integer component of Y offset in line
    pub y_int: i32,
    /// Optional color override
    pub color_opt: Option<Color>,
}

/// A line of laid out glyphs
pub struct LayoutLine {
    /// Glyphs in line
    pub glyphs: Vec<LayoutGlyph>,
}
