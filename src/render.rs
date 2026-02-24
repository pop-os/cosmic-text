//! Helpers for rendering buffers and editors

use crate::{Color, LayoutGlyph, LayoutRun, PhysicalGlyph, TextDecoration, UnderlineStyle};
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

pub fn render_decoration<R: Renderer>(renderer: &mut R, run: &LayoutRun, default_color: Color) {
    if run.glyphs.is_empty() {
        return;
    }

    let mut group_start: Option<usize> = None;

    for (i, glyph) in run.glyphs.iter().enumerate() {
        let start_new_group = match group_start {
            None => true,
            Some(_) => {
                let prev = &run.glyphs[i - 1];
                glyph.text_decoration != prev.text_decoration
            }
        };

        if start_new_group {
            if let Some(gs) = group_start {
                draw_decoration_group(renderer, run, &run.glyphs[gs..i], default_color);
            }
            group_start = if has_any_decoration(&glyph.text_decoration) {
                Some(i)
            } else {
                None
            };
        }
    }

    if let Some(gs) = group_start {
        draw_decoration_group(renderer, run, &run.glyphs[gs..], default_color);
    }
}

fn has_any_decoration(td: &TextDecoration) -> bool {
    td.underline != UnderlineStyle::None || td.overline || td.strikethrough
}

fn draw_decoration_group<R: Renderer>(
    renderer: &mut R,
    run: &LayoutRun,
    glyphs: &[LayoutGlyph],
    default_color: Color,
) {
    if glyphs.is_empty() {
        return;
    }

    let first = &glyphs[0];
    let last = &glyphs[glyphs.len() - 1];
    let td = &glyphs[0].text_decoration;
    let font_size = first.font_size;
    let x_start = first.x;
    let x_end = last.x + last.w;
    let width = x_end - x_start;
    if width <= 0.0 {
        // first check to see if it's below 0.0
        return;
    }
    let width = width as u32;
    if width == 0 {
        return;
    }
    // Underline
    match td.underline {
        UnderlineStyle::None => {}
        UnderlineStyle::Single => {
            let color = td
                .underline_color_opt
                .or(first.color_opt)
                .unwrap_or(default_color);
            let thickness = (first.underline_metrics.thickness * font_size)
                .max(1.0)
                .ceil();
            let y = run.line_y - first.underline_metrics.offset * font_size;
            renderer.rectangle(x_start as i32, y as i32, width, thickness as u32, color);
        }
        UnderlineStyle::Double => {
            let color = td
                .underline_color_opt
                .or(first.color_opt)
                .unwrap_or(default_color);
            let thickness = (first.underline_metrics.thickness * font_size)
                .max(1.0)
                .ceil();
            let gap = thickness;
            let y = run.line_y - first.underline_metrics.offset * font_size;
            renderer.rectangle(x_start as i32, y as i32, width, thickness as u32, color);
            renderer.rectangle(
                x_start as i32,
                (y + thickness + gap) as i32,
                width,
                thickness as u32,
                color,
            );
        }
    }

    // Strikethrough
    if td.strikethrough {
        let color = td
            .strikethrough_color_opt
            .or(first.color_opt)
            .unwrap_or(default_color);
        let thickness = (first.strikethrough_metrics.thickness * font_size)
            .max(1.0)
            .ceil();
        let y = run.line_y - first.strikethrough_metrics.offset * font_size;
        renderer.rectangle(x_start as i32, y as i32, width, thickness as u32, color);
    }

    // Overline
    if td.overline {
        let color = td
            .overline_color_opt
            .or(first.color_opt)
            .unwrap_or(default_color);
        // we're reusing underline thickness for overline
        let thickness = (first.underline_metrics.thickness * font_size)
            .max(1.0)
            .ceil();
        let y = run.line_top; //TODO: this should be run.line_y - ascent
                              // but we don't have ascent in GlyphLayout
                              // using line_top as an approximation for now, which should be good enough for most fonts
        renderer.rectangle(x_start as i32, y as i32, width, thickness as u32, color);
    }
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
