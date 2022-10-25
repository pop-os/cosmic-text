// SPDX-License-Identifier: MIT OR Apache-2.0

use super::CacheKey;

pub struct FontLayoutGlyph {
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub w: f32,
    pub rtl: bool,
    pub cache_key: CacheKey,
    pub x_int: i32,
    pub y_int: i32,
}

pub struct FontLayoutLine {
    pub rtl: bool,
    pub glyphs: Vec<FontLayoutGlyph>,
}
