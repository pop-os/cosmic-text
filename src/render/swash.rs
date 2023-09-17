// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Color, Draw, FontSystem, LayoutRun, SwashCache};

impl Draw for SwashCache {
    fn draw_line<F>(
        &mut self,
        font_system: &mut FontSystem,
        run: &LayoutRun,
        color: Color,
        f: &mut F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        for glyph in run.glyphs.iter() {
            let physical_glyph = glyph.physical((0., 0.), 1.0);

            let glyph_color = match glyph.color_opt {
                Some(some) => some,
                None => color,
            };

            self.with_pixels(
                font_system,
                physical_glyph.cache_key,
                glyph_color,
                |x, y, color| {
                    f(
                        physical_glyph.x + x,
                        run.line_y as i32 + physical_glyph.y + y,
                        1,
                        1,
                        color,
                    );
                },
            );
        }
    }
}
