// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    cmp,
    fmt,
    time::Instant,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Attrs, AttrsList, BufferLine, Color, FontSystem, LayoutGlyph, LayoutLine, ShapeLine};

/// Current cursor location
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cursor {
    /// Text line the cursor is on
    pub line: usize,
    /// Index of glyph at cursor (will insert behind this glyph)
    pub index: usize,
}

impl Cursor {
    /// Create a new cursor
    pub const fn new(line: usize, index: usize) -> Self {
        Self { line, index }
    }
}

pub struct LayoutCursor {
    pub line: usize,
    pub layout: usize,
    pub glyph: usize,
}

impl LayoutCursor {
    pub fn new(line: usize, layout: usize, glyph: usize) -> Self {
        Self { line, layout, glyph }
    }
}

/// A line of visible text for rendering
pub struct LayoutRun<'a> {
    /// The index of the original text line
    pub line_i: usize,
    /// The original text line
    pub text: &'a str,
    /// True if the original paragraph direction is RTL
    pub rtl: bool,
    /// The array of layout glyphs to draw
    pub glyphs: &'a [LayoutGlyph],
    /// Y offset of line
    pub line_y: i32,
}

/// An iterator of visible text lines, see [LayoutRun]
pub struct LayoutRunIter<'a, 'b> {
    buffer: &'b Buffer<'a>,
    line_i: usize,
    layout_i: usize,
    line_y: i32,
    total_layout: i32,
}

impl<'a, 'b> LayoutRunIter<'a, 'b> {
    pub fn new(buffer: &'b Buffer<'a>) -> Self {
        Self {
            buffer,
            line_i: 0,
            layout_i: 0,
            line_y: buffer.metrics.font_size - buffer.metrics.line_height,
            total_layout: 0,
        }
    }
}

impl<'a, 'b> Iterator for LayoutRunIter<'a, 'b> {
    type Item = LayoutRun<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.buffer.lines.get(self.line_i) {
            let shape = line.shape_opt().as_ref()?;
            let layout = line.layout_opt().as_ref()?;
            while let Some(layout_line) = layout.get(self.layout_i) {
                self.layout_i += 1;

                let scrolled = self.total_layout < self.buffer.scroll;
                self.total_layout += 1;
                if scrolled {
                    continue;
                }

                self.line_y += self.buffer.metrics.line_height;
                if self.line_y > self.buffer.height {
                    return None;
                }

                return Some(LayoutRun {
                    line_i: self.line_i,
                    text: line.text(),
                    rtl: shape.rtl,
                    glyphs: &layout_line.glyphs,
                    line_y: self.line_y,
                });
            }
            self.line_i += 1;
            self.layout_i = 0;
        }

        None
    }
}

/// Metrics of text
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Metrics {
    /// Font size in pixels
    pub font_size: i32,
    /// Line height in pixels
    pub line_height: i32,
}

impl Metrics {
    pub const fn new(font_size: i32, line_height: i32) -> Self {
        Self { font_size, line_height }
    }

    pub const fn scale(self, scale: i32) -> Self {
        Self {
            font_size: self.font_size * scale,
            line_height: self.line_height * scale,
        }
    }
}

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}px / {}px", self.font_size, self.line_height)
    }
}

/// A buffer of text that is shaped and laid out
pub struct Buffer<'a> {
    font_system: &'a FontSystem,
    /// Lines (or paragraphs) of text in the buffer
    pub lines: Vec<BufferLine>,
    metrics: Metrics,
    width: i32,
    height: i32,
    scroll: i32,
    /// True if a redraw is requires. Set to false after processing
    pub redraw: bool,
}

impl<'a> Buffer<'a> {
    /// Create a new [Buffer] with the provided [FontSystem] and [Metrics]
    pub fn new(
        font_system: &'a FontSystem,
        metrics: Metrics,
    ) -> Self {
        let mut buffer = Self {
            font_system,
            lines: Vec::new(),
            metrics,
            width: 0,
            height: 0,
            scroll: 0,
            redraw: false,
        };
        buffer.set_text("", Attrs::new());
        buffer
    }

    fn relayout(&mut self) {
        let instant = Instant::now();

        for line in self.lines.iter_mut() {
            if line.shape_opt().is_some() {
                line.reset_layout();
                line.layout(
                    self.font_system,
                    self.metrics.font_size,
                    self.width
                );
            }
        }

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout: {:?}", duration);
    }

    /// Pre-shape lines in the buffer, up to `lines`, return actual number of layout lines
    pub fn shape_until(&mut self, lines: i32) -> i32 {
        let instant = Instant::now();

        let mut reshaped = 0;
        let mut total_layout = 0;
        for line in self.lines.iter_mut() {
            if total_layout >= lines {
                break;
            }

            if line.shape_opt().is_none() {
                reshaped += 1;
            }
            let layout = line.layout(
                self.font_system,
                self.metrics.font_size,
                self.width
            );
            total_layout += layout.len() as i32;
        }

        let duration = instant.elapsed();
        if reshaped > 0 {
            log::debug!("shape_until {}: {:?}", reshaped, duration);
            self.redraw = true;
        }

        total_layout
    }

    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(&mut self, cursor: Cursor) {
        let instant = Instant::now();

        let mut reshaped = 0;
        let mut layout_i = 0;
        for (line_i, line) in self.lines.iter_mut().enumerate() {
            if line_i > cursor.line {
                break;
            }

            if line.shape_opt().is_none() {
                reshaped += 1;
            }
            let layout = line.layout(
                self.font_system,
                self.metrics.font_size,
                self.width
            );
            if line_i == cursor.line {
                let layout_cursor = self.layout_cursor(&cursor);
                layout_i += layout_cursor.layout as i32;
                break;
            } else {
                layout_i += layout.len() as i32;
            }
        }

        let duration = instant.elapsed();
        if reshaped > 0 {
            log::debug!("shape_until_cursor {}: {:?}", reshaped, duration);
            self.redraw = true;
        }

        let lines = self.visible_lines();
        if layout_i < self.scroll {
            self.scroll = layout_i;
        } else if layout_i >= self.scroll + lines {
            self.scroll = layout_i - (lines - 1);
        }

        self.shape_until_scroll();
    }

    /// Shape lines until scroll
    pub fn shape_until_scroll(&mut self) {
        let lines = self.visible_lines();

        let scroll_end = self.scroll + lines;
        let total_layout = self.shape_until(scroll_end);

        self.scroll = cmp::max(
            0,
            cmp::min(
                total_layout - (lines - 1),
                self.scroll,
            ),
        );
    }

    pub fn layout_cursor(&self, cursor: &Cursor) -> LayoutCursor {
        let line = &self.lines[cursor.line];

        let layout = line.layout_opt().as_ref().unwrap(); //TODO: ensure layout is done?
        for (layout_i, layout_line) in layout.iter().enumerate() {
            for (glyph_i, glyph) in layout_line.glyphs.iter().enumerate() {
                if cursor.index == glyph.start {
                    return LayoutCursor::new(
                        cursor.line,
                        layout_i,
                        glyph_i
                    );
                }
            }
            match layout_line.glyphs.last() {
                Some(glyph) => {
                    if cursor.index == glyph.end {
                        return LayoutCursor::new(
                            cursor.line,
                            layout_i,
                            layout_line.glyphs.len()
                        );
                    }
                },
                None => {
                    return LayoutCursor::new(
                        cursor.line,
                        layout_i,
                        0
                    );
                }
            }
        }

        // Fall back to start of line
        //TODO: should this be the end of the line?
        LayoutCursor::new(
            cursor.line,
            0,
            0
        )
    }

    /// Shape the provided line index and return the result
    pub fn line_shape(&mut self, line_i: usize) -> Option<&ShapeLine> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.shape(&self.font_system))
    }

    /// Lay out the provided line index and return the result
    pub fn line_layout(&mut self, line_i: usize) -> Option<&[LayoutLine]> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.layout(&self.font_system, self.metrics.font_size, self.width))
    }

    /// Get the current [Metrics]
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// Set the current [Metrics]
    pub fn set_metrics(&mut self, metrics: Metrics) {
        if metrics != self.metrics {
            self.metrics = metrics;
            self.relayout();
            self.shape_until_scroll();
        }
    }

    /// Get the current buffer dimensions (width, height)
    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    /// Set the current buffer dimensions
    pub fn set_size(&mut self, width: i32, height: i32) {
        if width != self.width {
            self.width = width;
            self.relayout();
            self.shape_until_scroll();
        }

        if height != self.height {
            self.height = height;
            self.shape_until_scroll();
        }
    }

    /// Get the current scroll location
    pub fn scroll(&self) -> i32 {
        self.scroll
    }

    /// Set the current scroll location
    pub fn set_scroll(&mut self, scroll: i32) {
        if scroll != self.scroll {
            self.scroll = scroll;
            self.redraw = true;
        }
    }

    /// Get the number of lines that can be viewed in the buffer
    pub fn visible_lines(&self) -> i32 {
        self.height / self.metrics.line_height
    }

    /// Set text of buffer, using provided attributes for each line by default
    pub fn set_text(&mut self, text: &str, attrs: Attrs<'a>) {
        self.lines.clear();
        for line in text.lines() {
            self.lines.push(BufferLine::new(line.to_string(), AttrsList::new(attrs)));
        }
        // Make sure there is always one line
        if self.lines.is_empty() {
            self.lines.push(BufferLine::new(String::new(), AttrsList::new(attrs)));
        }

        self.scroll = 0;

        self.shape_until_scroll();
    }

    /// Get the visible layout runs for rendering and other tasks
    pub fn layout_runs<'b>(&'b self) -> LayoutRunIter<'a, 'b> {
        LayoutRunIter::new(self)
    }

    /// Convert x, y position to Cursor (hit detection)
    pub fn hit(&self, x: i32, y: i32) -> Option<Cursor> {
        let instant = Instant::now();

        let font_size = self.metrics.font_size;
        let line_height = self.metrics.line_height;

        let mut new_cursor_opt = None;

        let mut runs = self.layout_runs().peekable();
        while let Some(run) = runs.next() {
            let line_y = run.line_y;

            if y >= line_y - font_size
            && y < line_y - font_size + line_height
            {
                let mut new_cursor_glyph = run.glyphs.len();
                let mut new_cursor_char = 0;
                'hit: for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                    if x >= glyph.x as i32
                    && x <= (glyph.x + glyph.w) as i32
                    {
                        new_cursor_glyph = glyph_i;

                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count();
                        let mut egc_x = glyph.x;
                        let egc_w = glyph.w / (total as f32);
                        for (egc_i, egc) in cluster.grapheme_indices(true) {
                            if x >= egc_x as i32
                            && x <= (egc_x + egc_w) as i32
                            {
                                new_cursor_char = egc_i;

                                let right_half = x >= (egc_x + egc_w / 2.0) as i32;
                                if right_half != glyph.rtl {
                                    // If clicking on last half of glyph, move cursor past glyph
                                    new_cursor_char += egc.len();
                                }
                                break 'hit;
                            }
                            egc_x += egc_w;
                        }

                        let right_half = x >= (glyph.x + glyph.w / 2.0) as i32;
                        if right_half != glyph.rtl {
                            // If clicking on last half of glyph, move cursor past glyph
                            new_cursor_char = cluster.len();
                        }
                        break 'hit;
                    }
                }

                let mut new_cursor = Cursor::new(run.line_i, 0);

                match run.glyphs.get(new_cursor_glyph) {
                    Some(glyph) => {
                        // Position at glyph
                        new_cursor.index = glyph.start + new_cursor_char;
                    },
                    None => if let Some(glyph) = run.glyphs.last() {
                        // Position at end of line
                        new_cursor.index = glyph.end;
                    },
                }

                new_cursor_opt = Some(new_cursor);

                break;
            } else if runs.peek().is_none() && y > run.line_y {
                let mut new_cursor = Cursor::new(run.line_i, 0);
                if let Some(glyph) = run.glyphs.last() {
                    new_cursor.index = glyph.end;
                }
                new_cursor_opt = Some(new_cursor);
            }
        }

        let duration = instant.elapsed();
        log::trace!("click({}, {}): {:?}", x, y, duration);

        new_cursor_opt
    }

    /// Draw the buffer
    #[cfg(feature = "swash")]
    pub fn draw<F>(&self, cache: &mut crate::SwashCache, color: Color, mut f: F)
        where F: FnMut(i32, i32, u32, u32, Color)
    {
        for run in self.layout_runs() {
            for glyph in run.glyphs.iter() {
                let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => color,
                };

                cache.with_pixels(cache_key, glyph_color, |x, y, color| {
                    f(x_int + x, run.line_y + y_int + y, 1, 1, color)
                });
            }
        }
    }
}
