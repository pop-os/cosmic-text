//! Helpers for rendering buffers and editors

use crate::{Color, PhysicalGlyph};
#[cfg(feature = "swash")]
use crate::{FontSystem, SwashCache};

/// Custom renderer for buffers and editors
pub trait Renderer {
    /// Render a rectangle at x, y with size w, h and the provided [`Color`].
    fn rectangle(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color);

    /// Render a [`PhysicalGlyph`] with the provided [`Color`].
    /// For performance, consider using [`SwashCache`].
    fn glyph(&mut self, physical_glyph: PhysicalGlyph, color: Color);
}

/// Helper to migrate from old renderer
//TODO: remove in future version
#[cfg(feature = "swash")]
#[derive(Debug)]
pub struct LegacyRenderer<'a, F: FnMut(i32, i32, u32, u32, Color)> {
    pub font_system: &'a mut FontSystem,
    pub cache: &'a mut SwashCache,
    pub callback: F,
}

#[cfg(feature = "swash")]
impl<'a, F: FnMut(i32, i32, u32, u32, Color)> Renderer for LegacyRenderer<'a, F> {
    fn rectangle(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        (self.callback)(x, y, w, h, color);
    }

    fn glyph(&mut self, physical_glyph: PhysicalGlyph, color: Color) {
        self.cache.with_pixels(
            self.font_system,
            physical_glyph.cache_key,
            color,
            |x, y, pixel_color| {
                (self.callback)(
                    physical_glyph.x + x,
                    physical_glyph.y + y,
                    1,
                    1,
                    pixel_color,
                );
            },
        );
    }
}
