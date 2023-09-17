// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Color, FontSystem, LayoutRun};

/// A trait to represent glyph renderer
pub trait Draw {
    /// Draw a line of laid out glyphs
    fn draw_line<F>(
        &mut self,
        font_system: &mut FontSystem,
        run: &LayoutRun<'_>,
        color: Color,
        f: &mut F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color);
}

#[cfg(feature = "swash")]
mod swash;
#[cfg(feature = "swash")]
pub use self::swash::*;
