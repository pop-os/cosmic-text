// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::{cmp, fmt};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Affinity, Attrs, AttrsList, BidiParagraphs, BorrowedWithFontSystem, BufferLine, Color, Cursor,
    FontSystem, LayoutCursor, LayoutGlyph, LayoutLine, Motion, Scroll, ShapeBuffer, ShapeLine,
    Shaping, Wrap,
};

/// A line of visible text for rendering
#[derive(Debug)]
pub struct LayoutRun<'a> {
    /// The index of the original text line
    pub line_i: usize,
    /// The original text line
    pub text: &'a str,
    /// True if the original paragraph direction is RTL
    pub rtl: bool,
    /// The array of layout glyphs to draw
    pub glyphs: &'a [LayoutGlyph],
    /// Y offset to baseline of line
    pub line_y: f32,
    /// Y offset to top of line
    pub line_top: f32,
    /// Width of line
    pub line_w: f32,
}

impl<'a> LayoutRun<'a> {
    /// Return the pixel span `Some((x_left, x_width))` of the highlighted area between `cursor_start`
    /// and `cursor_end` within this run, or None if the cursor range does not intersect this run.
    /// This may return widths of zero if `cursor_start == cursor_end`, if the run is empty, or if the
    /// region's left start boundary is the same as the cursor's end boundary or vice versa.
    pub fn highlight(&self, cursor_start: Cursor, cursor_end: Cursor) -> Option<(f32, f32)> {
        let mut x_start = None;
        let mut x_end = None;
        let rtl_factor = if self.rtl { 1. } else { 0. };
        let ltr_factor = 1. - rtl_factor;
        for glyph in self.glyphs.iter() {
            let cursor = self.cursor_from_glyph_left(glyph);
            if cursor >= cursor_start && cursor <= cursor_end {
                if x_start.is_none() {
                    x_start = Some(glyph.x + glyph.w * rtl_factor);
                }
                x_end = Some(glyph.x + glyph.w * rtl_factor);
            }
            let cursor = self.cursor_from_glyph_right(glyph);
            if cursor >= cursor_start && cursor <= cursor_end {
                if x_start.is_none() {
                    x_start = Some(glyph.x + glyph.w * ltr_factor);
                }
                x_end = Some(glyph.x + glyph.w * ltr_factor);
            }
        }
        if let Some(x_start) = x_start {
            let x_end = x_end.expect("end of cursor not found");
            let (x_start, x_end) = if x_start < x_end {
                (x_start, x_end)
            } else {
                (x_end, x_start)
            };
            Some((x_start, x_end - x_start))
        } else {
            None
        }
    }

    fn cursor_from_glyph_left(&self, glyph: &LayoutGlyph) -> Cursor {
        if self.rtl {
            Cursor::new_with_affinity(self.line_i, glyph.end, Affinity::Before)
        } else {
            Cursor::new_with_affinity(self.line_i, glyph.start, Affinity::After)
        }
    }

    fn cursor_from_glyph_right(&self, glyph: &LayoutGlyph) -> Cursor {
        if self.rtl {
            Cursor::new_with_affinity(self.line_i, glyph.start, Affinity::After)
        } else {
            Cursor::new_with_affinity(self.line_i, glyph.end, Affinity::Before)
        }
    }
}

/// An iterator of visible text lines, see [`LayoutRun`]
#[derive(Debug)]
pub struct LayoutRunIter<'b> {
    buffer: &'b Buffer,
    line_i: usize,
    layout_i: usize,
    remaining_len: usize,
    total_layout: i32,
}

impl<'b> LayoutRunIter<'b> {
    pub fn new(buffer: &'b Buffer) -> Self {
        let total_layout_lines: usize = buffer
            .lines
            .iter()
            .skip(buffer.scroll.line)
            .map(|line| {
                line.layout_opt()
                    .as_ref()
                    .map(|layout| layout.len())
                    .unwrap_or_default()
            })
            .sum();
        let top_cropped_layout_lines =
            total_layout_lines.saturating_sub(buffer.scroll.layout.try_into().unwrap_or_default());
        let maximum_lines = if buffer.metrics.line_height == 0.0 {
            0
        } else {
            (buffer.height / buffer.metrics.line_height) as i32
        };
        let bottom_cropped_layout_lines =
            if top_cropped_layout_lines > maximum_lines.try_into().unwrap_or_default() {
                maximum_lines.try_into().unwrap_or_default()
            } else {
                top_cropped_layout_lines
            };

        Self {
            buffer,
            line_i: buffer.scroll.line,
            layout_i: 0,
            remaining_len: bottom_cropped_layout_lines,
            total_layout: 0,
        }
    }
}

impl<'b> Iterator for LayoutRunIter<'b> {
    type Item = LayoutRun<'b>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining_len, Some(self.remaining_len))
    }

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.buffer.lines.get(self.line_i) {
            let shape = line.shape_opt().as_ref()?;
            let layout = line.layout_opt().as_ref()?;
            while let Some(layout_line) = layout.get(self.layout_i) {
                self.layout_i += 1;

                let scrolled = self.total_layout < self.buffer.scroll.layout;
                self.total_layout += 1;
                if scrolled {
                    continue;
                }

                let line_top = self
                    .total_layout
                    .saturating_sub(self.buffer.scroll.layout)
                    .saturating_sub(1) as f32
                    * self.buffer.metrics.line_height;
                let glyph_height = layout_line.max_ascent + layout_line.max_descent;
                let centering_offset = (self.buffer.metrics.line_height - glyph_height) / 2.0;
                let line_y = line_top + centering_offset + layout_line.max_ascent;

                if line_top + centering_offset > self.buffer.height {
                    return None;
                }

                return self.remaining_len.checked_sub(1).map(|num| {
                    self.remaining_len = num;
                    LayoutRun {
                        line_i: self.line_i,
                        text: line.text(),
                        rtl: shape.rtl,
                        glyphs: &layout_line.glyphs,
                        line_y,
                        line_top,
                        line_w: layout_line.w,
                    }
                });
            }
            self.line_i += 1;
            self.layout_i = 0;
        }

        None
    }
}

impl<'b> ExactSizeIterator for LayoutRunIter<'b> {}

/// Metrics of text
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    /// Font size in pixels
    pub font_size: f32,
    /// Line height in pixels
    pub line_height: f32,
}

impl Metrics {
    pub const fn new(font_size: f32, line_height: f32) -> Self {
        Self {
            font_size,
            line_height,
        }
    }

    pub fn scale(self, scale: f32) -> Self {
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
#[derive(Debug)]
pub struct Buffer {
    /// [BufferLine]s (or paragraphs) of text in the buffer
    pub lines: Vec<BufferLine>,
    metrics: Metrics,
    width: f32,
    height: f32,
    scroll: Scroll,
    /// True if a redraw is requires. Set to false after processing
    redraw: bool,
    wrap: Wrap,
    monospace_width: Option<f32>,

    /// Scratch buffer for shaping and laying out.
    scratch: ShapeBuffer,
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            metrics: self.metrics,
            width: self.width,
            height: self.height,
            scroll: self.scroll,
            redraw: self.redraw,
            wrap: self.wrap,
            monospace_width: self.monospace_width,
            scratch: ShapeBuffer::default(),
        }
    }
}

impl Buffer {
    /// Create an empty [`Buffer`] with the provided [`Metrics`].
    /// This is useful for initializing a [`Buffer`] without a [`FontSystem`].
    ///
    /// You must populate the [`Buffer`] with at least one [`BufferLine`] before shaping and layout,
    /// for example by calling [`Buffer::set_text`].
    ///
    /// If you have a [`FontSystem`] in scope, you should use [`Buffer::new`] instead.
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.line_height` is zero.
    pub fn new_empty(metrics: Metrics) -> Self {
        assert_ne!(metrics.line_height, 0.0, "line height cannot be 0");
        Self {
            lines: Vec::new(),
            metrics,
            width: 0.0,
            height: 0.0,
            scroll: Scroll::default(),
            redraw: false,
            wrap: Wrap::Word,
            scratch: ShapeBuffer::default(),
            monospace_width: None,
        }
    }

    /// Create a new [`Buffer`] with the provided [`FontSystem`] and [`Metrics`]
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.line_height` is zero.
    pub fn new(font_system: &mut FontSystem, metrics: Metrics) -> Self {
        let mut buffer = Self::new_empty(metrics);
        buffer.set_text(font_system, "", Attrs::new(), Shaping::Advanced);
        buffer
    }

    /// Mutably borrows the buffer together with an [`FontSystem`] for more convenient methods
    pub fn borrow_with<'a>(
        &'a mut self,
        font_system: &'a mut FontSystem,
    ) -> BorrowedWithFontSystem<'a, Buffer> {
        BorrowedWithFontSystem {
            inner: self,
            font_system,
        }
    }

    fn relayout(&mut self, font_system: &mut FontSystem) {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        for line in &mut self.lines {
            if line.shape_opt().is_some() {
                line.reset_layout();
                line.layout_in_buffer(
                    &mut self.scratch,
                    font_system,
                    self.metrics.font_size,
                    self.width,
                    self.wrap,
                    self.monospace_width,
                );
            }
        }

        self.redraw = true;

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::debug!("relayout: {:?}", instant.elapsed());
    }

    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(
        &mut self,
        font_system: &mut FontSystem,
        cursor: Cursor,
        prune: bool,
    ) {
        let old_scroll = self.scroll;

        let layout_cursor = self
            .layout_cursor(font_system, cursor)
            .expect("shape_until_cursor invalid cursor");

        if self.scroll.line > layout_cursor.line
            || (self.scroll.line == layout_cursor.line
                && self.scroll.layout > layout_cursor.layout as i32)
        {
            // Adjust scroll backwards if cursor is before it
            self.scroll.line = layout_cursor.line;
            self.scroll.layout = layout_cursor.layout as i32;
        } else {
            // Adjust scroll forwards if cursor is after it
            let visible_lines = self.visible_lines();
            let mut line_i = layout_cursor.line;
            let mut total_layout = layout_cursor.layout as i32 + 1;
            while line_i > self.scroll.line {
                line_i -= 1;
                let layout = self
                    .line_layout(font_system, line_i)
                    .expect("shape_until_cursor failed to scroll forwards");
                for layout_i in (0..layout.len()).rev() {
                    total_layout += 1;
                    if total_layout >= visible_lines {
                        self.scroll.line = line_i;
                        self.scroll.layout = layout_i as i32;
                        break;
                    }
                }
            }
        }

        if old_scroll != self.scroll {
            self.redraw = true;
        }

        self.shape_until_scroll(font_system, prune);
    }

    /// Shape lines until scroll
    pub fn shape_until_scroll(&mut self, font_system: &mut FontSystem, prune: bool) {
        let old_scroll = self.scroll;

        loop {
            // Adjust scroll.layout to be positive by moving scroll.line backwards
            while self.scroll.layout < 0 {
                if self.scroll.line > 0 {
                    self.scroll.line -= 1;
                    if let Some(layout) = self.line_layout(font_system, self.scroll.line) {
                        self.scroll.layout += layout.len() as i32;
                    }
                } else {
                    self.scroll.layout = 0;
                    break;
                }
            }

            let visible_lines = self.visible_lines();
            let scroll_start = self.scroll.layout;
            let scroll_end = scroll_start + visible_lines;

            let mut total_layout = 0;
            for line_i in 0..self.lines.len() {
                if line_i < self.scroll.line {
                    if prune {
                        self.lines[line_i].reset_shaping();
                    }
                    continue;
                }
                if total_layout >= scroll_end {
                    if prune {
                        self.lines[line_i].reset_shaping();
                        continue;
                    } else {
                        break;
                    }
                }

                let layout = self
                    .line_layout(font_system, line_i)
                    .expect("shape_until_scroll invalid line");
                for layout_i in 0..layout.len() {
                    if total_layout == scroll_start {
                        // Adjust scroll.line and scroll.layout
                        self.scroll.line = line_i;
                        self.scroll.layout = layout_i as i32;
                    }
                    total_layout += 1;
                }
            }

            if total_layout < scroll_end && self.scroll.line > 0 {
                // Need to scroll up to stay inside of buffer
                self.scroll.layout -= scroll_end - total_layout;
            } else {
                // Done adjusting scroll
                break;
            }
        }

        if old_scroll != self.scroll {
            self.redraw = true;
        }
    }

    /// Convert a [`Cursor`] to a [`LayoutCursor`]
    pub fn layout_cursor(
        &mut self,
        font_system: &mut FontSystem,
        cursor: Cursor,
    ) -> Option<LayoutCursor> {
        let layout = self.line_layout(font_system, cursor.line)?;
        for (layout_i, layout_line) in layout.iter().enumerate() {
            for (glyph_i, glyph) in layout_line.glyphs.iter().enumerate() {
                let cursor_end =
                    Cursor::new_with_affinity(cursor.line, glyph.end, Affinity::Before);
                let cursor_start =
                    Cursor::new_with_affinity(cursor.line, glyph.start, Affinity::After);
                let (cursor_left, cursor_right) = if glyph.level.is_ltr() {
                    (cursor_start, cursor_end)
                } else {
                    (cursor_end, cursor_start)
                };
                if cursor == cursor_left {
                    return Some(LayoutCursor::new(cursor.line, layout_i, glyph_i));
                }
                if cursor == cursor_right {
                    return Some(LayoutCursor::new(cursor.line, layout_i, glyph_i + 1));
                }
            }
        }

        // Fall back to start of line
        //TODO: should this be the end of the line?
        Some(LayoutCursor::new(cursor.line, 0, 0))
    }

    /// Shape the provided line index and return the result
    pub fn line_shape(
        &mut self,
        font_system: &mut FontSystem,
        line_i: usize,
    ) -> Option<&ShapeLine> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.shape_in_buffer(&mut self.scratch, font_system))
    }

    /// Lay out the provided line index and return the result
    pub fn line_layout(
        &mut self,
        font_system: &mut FontSystem,
        line_i: usize,
    ) -> Option<&[LayoutLine]> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.layout_in_buffer(
            &mut self.scratch,
            font_system,
            self.metrics.font_size,
            self.width,
            self.wrap,
            self.monospace_width,
        ))
    }

    /// Get the current [`Metrics`]
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// Set the current [`Metrics`]
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.font_size` is zero.
    pub fn set_metrics(&mut self, font_system: &mut FontSystem, metrics: Metrics) {
        self.set_metrics_and_size(font_system, metrics, self.width, self.height);
    }

    /// Get the current [`Wrap`]
    pub fn wrap(&self) -> Wrap {
        self.wrap
    }

    /// Set the current [`Wrap`]
    pub fn set_wrap(&mut self, font_system: &mut FontSystem, wrap: Wrap) {
        if wrap != self.wrap {
            self.wrap = wrap;
            self.relayout(font_system);
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current `monospace_width`
    pub fn monospace_width(&self) -> Option<f32> {
        self.monospace_width
    }

    /// Set monospace width monospace glyphs should be resized to match. `None` means don't resize
    pub fn set_monospace_width(&mut self, font_system: &mut FontSystem, monospace_width: Option<f32>) {
        if monospace_width != self.monospace_width {
            self.monospace_width = monospace_width;
            self.relayout(font_system);
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current buffer dimensions (width, height)
    pub fn size(&self) -> (f32, f32) {
        (self.width, self.height)
    }

    /// Set the current buffer dimensions
    pub fn set_size(&mut self, font_system: &mut FontSystem, width: f32, height: f32) {
        self.set_metrics_and_size(font_system, self.metrics, width, height);
    }

    /// Set the current [`Metrics`] and buffer dimensions at the same time
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.font_size` is zero.
    pub fn set_metrics_and_size(
        &mut self,
        font_system: &mut FontSystem,
        metrics: Metrics,
        width: f32,
        height: f32,
    ) {
        let clamped_width = width.max(0.0);
        let clamped_height = height.max(0.0);

        if metrics != self.metrics || clamped_width != self.width || clamped_height != self.height {
            assert_ne!(metrics.font_size, 0.0, "font size cannot be 0");
            self.metrics = metrics;
            self.width = clamped_width;
            self.height = clamped_height;
            self.relayout(font_system);
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current scroll location
    pub fn scroll(&self) -> Scroll {
        self.scroll
    }

    /// Set the current scroll location
    pub fn set_scroll(&mut self, scroll: Scroll) {
        if scroll != self.scroll {
            self.scroll = scroll;
            self.redraw = true;
        }
    }

    /// Get the number of lines that can be viewed in the buffer
    pub fn visible_lines(&self) -> i32 {
        (self.height / self.metrics.line_height) as i32
    }

    /// Set text of buffer, using provided attributes for each line by default
    pub fn set_text(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
        attrs: Attrs,
        shaping: Shaping,
    ) {
        self.set_rich_text(font_system, [(text, attrs)], attrs, shaping);
    }

    /// Set text of buffer, using an iterator of styled spans (pairs of text and attributes)
    ///
    /// ```
    /// # use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
    /// # let mut font_system = FontSystem::new();
    /// let mut buffer = Buffer::new_empty(Metrics::new(32.0, 44.0));
    /// let attrs = Attrs::new().family(Family::Serif);
    /// buffer.set_rich_text(
    ///     &mut font_system,
    ///     [
    ///         ("hello, ", attrs),
    ///         ("cosmic\ntext", attrs.family(Family::Monospace)),
    ///     ],
    ///     attrs,
    ///     Shaping::Advanced,
    /// );
    /// ```
    pub fn set_rich_text<'r, 's, I>(
        &mut self,
        font_system: &mut FontSystem,
        spans: I,
        default_attrs: Attrs,
        shaping: Shaping,
    ) where
        I: IntoIterator<Item = (&'s str, Attrs<'r>)>,
    {
        self.lines.clear();

        let mut attrs_list = AttrsList::new(default_attrs);
        let mut line_string = String::new();
        let mut end = 0;
        let (string, spans_data): (String, Vec<_>) = spans
            .into_iter()
            .map(|(s, attrs)| {
                let start = end;
                end += s.len();
                (s, (attrs, start..end))
            })
            .unzip();

        let mut spans_iter = spans_data.into_iter();
        let mut maybe_span = spans_iter.next();

        // split the string into lines, as ranges
        let string_start = string.as_ptr() as usize;
        let mut lines_iter = BidiParagraphs::new(&string).map(|line: &str| {
            let start = line.as_ptr() as usize - string_start;
            let end = start + line.len();
            start..end
        });
        let mut maybe_line = lines_iter.next();

        loop {
            let (Some(line_range), Some((attrs, span_range))) = (&maybe_line, &maybe_span) else {
                // this is reached only if this text is empty
                self.lines.push(BufferLine::new(
                    String::new(),
                    AttrsList::new(default_attrs),
                    shaping,
                ));
                break;
            };

            // start..end is the intersection of this line and this span
            let start = line_range.start.max(span_range.start);
            let end = line_range.end.min(span_range.end);
            if start < end {
                let text = &string[start..end];
                let text_start = line_string.len();
                line_string.push_str(text);
                let text_end = line_string.len();
                // Only add attrs if they don't match the defaults
                if *attrs != attrs_list.defaults() {
                    attrs_list.add_span(text_start..text_end, *attrs);
                }
            }

            // we know that at the end of a line,
            // span text's end index is always >= line text's end index
            // so if this span ends before this line ends,
            // there is another span in this line.
            // otherwise, we move on to the next line.
            if span_range.end < line_range.end {
                maybe_span = spans_iter.next();
            } else {
                maybe_line = lines_iter.next();
                if maybe_line.is_some() {
                    // finalize this line and start a new line
                    let prev_attrs_list =
                        core::mem::replace(&mut attrs_list, AttrsList::new(default_attrs));
                    let prev_line_string = core::mem::take(&mut line_string);
                    let buffer_line = BufferLine::new(prev_line_string, prev_attrs_list, shaping);
                    self.lines.push(buffer_line);
                } else {
                    // finalize the final line
                    let buffer_line = BufferLine::new(line_string, attrs_list, shaping);
                    self.lines.push(buffer_line);
                    break;
                }
            }
        }

        self.scroll = Scroll::default();

        self.shape_until_scroll(font_system, false);
    }

    /// True if a redraw is needed
    pub fn redraw(&self) -> bool {
        self.redraw
    }

    /// Set redraw needed flag
    pub fn set_redraw(&mut self, redraw: bool) {
        self.redraw = redraw;
    }

    /// Get the visible layout runs for rendering and other tasks
    pub fn layout_runs(&self) -> LayoutRunIter {
        LayoutRunIter::new(self)
    }

    /// Convert x, y position to Cursor (hit detection)
    pub fn hit(&self, x: f32, y: f32) -> Option<Cursor> {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        let font_size = self.metrics.font_size;
        let line_height = self.metrics.line_height;

        let mut new_cursor_opt = None;

        let mut runs = self.layout_runs().peekable();
        let mut first_run = true;
        while let Some(run) = runs.next() {
            let line_y = run.line_y;

            if first_run && y < line_y - font_size {
                first_run = false;
                let new_cursor = Cursor::new(run.line_i, 0);
                new_cursor_opt = Some(new_cursor);
            } else if y >= line_y - font_size && y < line_y - font_size + line_height {
                let mut new_cursor_glyph = run.glyphs.len();
                let mut new_cursor_char = 0;
                let mut new_cursor_affinity = Affinity::After;

                let mut first_glyph = true;

                'hit: for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                    if first_glyph {
                        first_glyph = false;
                        if (run.rtl && x > glyph.x) || (!run.rtl && x < 0.0) {
                            new_cursor_glyph = 0;
                            new_cursor_char = 0;
                        }
                    }
                    if x >= glyph.x && x <= glyph.x + glyph.w {
                        new_cursor_glyph = glyph_i;

                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count();
                        let mut egc_x = glyph.x;
                        let egc_w = glyph.w / (total as f32);
                        for (egc_i, egc) in cluster.grapheme_indices(true) {
                            if x >= egc_x && x <= egc_x + egc_w {
                                new_cursor_char = egc_i;

                                let right_half = x >= egc_x + egc_w / 2.0;
                                if right_half != glyph.level.is_rtl() {
                                    // If clicking on last half of glyph, move cursor past glyph
                                    new_cursor_char += egc.len();
                                    new_cursor_affinity = Affinity::Before;
                                }
                                break 'hit;
                            }
                            egc_x += egc_w;
                        }

                        let right_half = x >= glyph.x + glyph.w / 2.0;
                        if right_half != glyph.level.is_rtl() {
                            // If clicking on last half of glyph, move cursor past glyph
                            new_cursor_char = cluster.len();
                            new_cursor_affinity = Affinity::Before;
                        }
                        break 'hit;
                    }
                }

                let mut new_cursor = Cursor::new(run.line_i, 0);

                match run.glyphs.get(new_cursor_glyph) {
                    Some(glyph) => {
                        // Position at glyph
                        new_cursor.index = glyph.start + new_cursor_char;
                        new_cursor.affinity = new_cursor_affinity;
                    }
                    None => {
                        if let Some(glyph) = run.glyphs.last() {
                            // Position at end of line
                            new_cursor.index = glyph.end;
                            new_cursor.affinity = Affinity::Before;
                        }
                    }
                }

                new_cursor_opt = Some(new_cursor);

                break;
            } else if runs.peek().is_none() && y > run.line_y {
                let mut new_cursor = Cursor::new(run.line_i, 0);
                if let Some(glyph) = run.glyphs.last() {
                    new_cursor = run.cursor_from_glyph_right(glyph);
                }
                new_cursor_opt = Some(new_cursor);
            }
        }

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::trace!("click({}, {}): {:?}", x, y, instant.elapsed());

        new_cursor_opt
    }

    /// Apply a [`Motion`] to a [`Cursor`]
    pub fn cursor_motion(
        &mut self,
        font_system: &mut FontSystem,
        mut cursor: Cursor,
        mut cursor_x_opt: Option<i32>,
        motion: Motion,
    ) -> Option<(Cursor, Option<i32>)> {
        match motion {
            Motion::LayoutCursor(layout_cursor) => {
                let layout = self.line_layout(font_system, layout_cursor.line)?;

                let layout_line = match layout.get(layout_cursor.layout) {
                    Some(some) => some,
                    None => match layout.last() {
                        Some(some) => some,
                        None => {
                            return None;
                        }
                    },
                };

                let (new_index, new_affinity) = match layout_line.glyphs.get(layout_cursor.glyph) {
                    Some(glyph) => (glyph.start, Affinity::After),
                    None => match layout_line.glyphs.last() {
                        Some(glyph) => (glyph.end, Affinity::Before),
                        //TODO: is this correct?
                        None => (0, Affinity::After),
                    },
                };

                if cursor.line != layout_cursor.line
                    || cursor.index != new_index
                    || cursor.affinity != new_affinity
                {
                    cursor.line = layout_cursor.line;
                    cursor.index = new_index;
                    cursor.affinity = new_affinity;
                }
            }
            Motion::Previous => {
                let line = self.lines.get(cursor.line)?;
                if cursor.index > 0 {
                    // Find previous character index
                    let mut prev_index = 0;
                    for (i, _) in line.text().grapheme_indices(true) {
                        if i < cursor.index {
                            prev_index = i;
                        } else {
                            break;
                        }
                    }

                    cursor.index = prev_index;
                    cursor.affinity = Affinity::After;
                } else if cursor.line > 0 {
                    cursor.line -= 1;
                    cursor.index = self.lines.get(cursor.line)?.text().len();
                    cursor.affinity = Affinity::After;
                }
                cursor_x_opt = None;
            }
            Motion::Next => {
                let line = self.lines.get(cursor.line)?;
                if cursor.index < line.text().len() {
                    for (i, c) in line.text().grapheme_indices(true) {
                        if i == cursor.index {
                            cursor.index += c.len();
                            cursor.affinity = Affinity::Before;
                            break;
                        }
                    }
                } else if cursor.line + 1 < self.lines.len() {
                    cursor.line += 1;
                    cursor.index = 0;
                    cursor.affinity = Affinity::Before;
                }
                cursor_x_opt = None;
            }
            Motion::Left => {
                let rtl_opt = self
                    .line_shape(font_system, cursor.line)
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        (cursor, cursor_x_opt) =
                            self.cursor_motion(font_system, cursor, cursor_x_opt, Motion::Next)?;
                    } else {
                        (cursor, cursor_x_opt) = self.cursor_motion(
                            font_system,
                            cursor,
                            cursor_x_opt,
                            Motion::Previous,
                        )?;
                    }
                }
            }
            Motion::Right => {
                let rtl_opt = self
                    .line_shape(font_system, cursor.line)
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        (cursor, cursor_x_opt) = self.cursor_motion(
                            font_system,
                            cursor,
                            cursor_x_opt,
                            Motion::Previous,
                        )?;
                    } else {
                        (cursor, cursor_x_opt) =
                            self.cursor_motion(font_system, cursor, cursor_x_opt, Motion::Next)?;
                    }
                }
            }
            Motion::Up => {
                let mut layout_cursor = self.layout_cursor(font_system, cursor)?;

                if cursor_x_opt.is_none() {
                    cursor_x_opt = Some(
                        layout_cursor.glyph as i32, //TODO: glyph x position
                    );
                }

                if layout_cursor.layout > 0 {
                    layout_cursor.layout -= 1;
                } else if layout_cursor.line > 0 {
                    layout_cursor.line -= 1;
                    layout_cursor.layout = usize::max_value();
                }

                if let Some(cursor_x) = cursor_x_opt {
                    layout_cursor.glyph = cursor_x as usize; //TODO: glyph x position
                }

                (cursor, cursor_x_opt) = self.cursor_motion(
                    font_system,
                    cursor,
                    cursor_x_opt,
                    Motion::LayoutCursor(layout_cursor),
                )?;
            }
            Motion::Down => {
                let mut layout_cursor = self.layout_cursor(font_system, cursor)?;

                let layout_len = self.line_layout(font_system, layout_cursor.line)?.len();

                if cursor_x_opt.is_none() {
                    cursor_x_opt = Some(
                        layout_cursor.glyph as i32, //TODO: glyph x position
                    );
                }

                if layout_cursor.layout + 1 < layout_len {
                    layout_cursor.layout += 1;
                } else if layout_cursor.line + 1 < self.lines.len() {
                    layout_cursor.line += 1;
                    layout_cursor.layout = 0;
                }

                if let Some(cursor_x) = cursor_x_opt {
                    layout_cursor.glyph = cursor_x as usize; //TODO: glyph x position
                }

                (cursor, cursor_x_opt) = self.cursor_motion(
                    font_system,
                    cursor,
                    cursor_x_opt,
                    Motion::LayoutCursor(layout_cursor),
                )?;
            }
            Motion::Home => {
                let mut layout_cursor = self.layout_cursor(font_system, cursor)?;
                layout_cursor.glyph = 0;
                #[allow(unused_assignments)]
                {
                    (cursor, cursor_x_opt) = self.cursor_motion(
                        font_system,
                        cursor,
                        cursor_x_opt,
                        Motion::LayoutCursor(layout_cursor),
                    )?;
                }
                cursor_x_opt = None;
            }
            Motion::SoftHome => {
                let line = self.lines.get(cursor.line)?;
                cursor.index = line
                    .text()
                    .char_indices()
                    .filter_map(|(i, c)| if c.is_whitespace() { None } else { Some(i) })
                    .next()
                    .unwrap_or(0);
                cursor_x_opt = None;
            }
            Motion::End => {
                let mut layout_cursor = self.layout_cursor(font_system, cursor)?;
                layout_cursor.glyph = usize::max_value();
                #[allow(unused_assignments)]
                {
                    (cursor, cursor_x_opt) = self.cursor_motion(
                        font_system,
                        cursor,
                        cursor_x_opt,
                        Motion::LayoutCursor(layout_cursor),
                    )?;
                }
                cursor_x_opt = None;
            }
            Motion::ParagraphStart => {
                cursor.index = 0;
                cursor_x_opt = None;
            }
            Motion::ParagraphEnd => {
                cursor.index = self.lines.get(cursor.line)?.text().len();
                cursor_x_opt = None;
            }
            Motion::PageUp => {
                (cursor, cursor_x_opt) = self.cursor_motion(
                    font_system,
                    cursor,
                    cursor_x_opt,
                    Motion::Vertical(-self.size().1 as i32),
                )?;
            }
            Motion::PageDown => {
                (cursor, cursor_x_opt) = self.cursor_motion(
                    font_system,
                    cursor,
                    cursor_x_opt,
                    Motion::Vertical(self.size().1 as i32),
                )?;
            }
            Motion::Vertical(px) => {
                // TODO more efficient
                let lines = px / self.metrics().line_height as i32;
                match lines.cmp(&0) {
                    cmp::Ordering::Less => {
                        for _ in 0..-lines {
                            (cursor, cursor_x_opt) =
                                self.cursor_motion(font_system, cursor, cursor_x_opt, Motion::Up)?;
                        }
                    }
                    cmp::Ordering::Greater => {
                        for _ in 0..lines {
                            (cursor, cursor_x_opt) = self.cursor_motion(
                                font_system,
                                cursor,
                                cursor_x_opt,
                                Motion::Down,
                            )?;
                        }
                    }
                    cmp::Ordering::Equal => {}
                }
            }
            Motion::PreviousWord => {
                let line = self.lines.get(cursor.line)?;
                if cursor.index > 0 {
                    cursor.index = line
                        .text()
                        .unicode_word_indices()
                        .rev()
                        .map(|(i, _)| i)
                        .find(|&i| i < cursor.index)
                        .unwrap_or(0);
                } else if cursor.line > 0 {
                    cursor.line -= 1;
                    cursor.index = self.lines.get(cursor.line)?.text().len();
                }
                cursor_x_opt = None;
            }
            Motion::NextWord => {
                let line = self.lines.get(cursor.line)?;
                if cursor.index < line.text().len() {
                    cursor.index = line
                        .text()
                        .unicode_word_indices()
                        .map(|(i, word)| i + word.len())
                        .find(|&i| i > cursor.index)
                        .unwrap_or(line.text().len());
                } else if cursor.line + 1 < self.lines.len() {
                    cursor.line += 1;
                    cursor.index = 0;
                }
                cursor_x_opt = None;
            }
            Motion::LeftWord => {
                let rtl_opt = self
                    .line_shape(font_system, cursor.line)
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        (cursor, cursor_x_opt) = self.cursor_motion(
                            font_system,
                            cursor,
                            cursor_x_opt,
                            Motion::NextWord,
                        )?;
                    } else {
                        (cursor, cursor_x_opt) = self.cursor_motion(
                            font_system,
                            cursor,
                            cursor_x_opt,
                            Motion::PreviousWord,
                        )?;
                    }
                }
            }
            Motion::RightWord => {
                let rtl_opt = self
                    .line_shape(font_system, cursor.line)
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        (cursor, cursor_x_opt) = self.cursor_motion(
                            font_system,
                            cursor,
                            cursor_x_opt,
                            Motion::PreviousWord,
                        )?;
                    } else {
                        (cursor, cursor_x_opt) = self.cursor_motion(
                            font_system,
                            cursor,
                            cursor_x_opt,
                            Motion::NextWord,
                        )?;
                    }
                }
            }
            Motion::BufferStart => {
                cursor.line = 0;
                cursor.index = 0;
                cursor_x_opt = None;
            }
            Motion::BufferEnd => {
                cursor.line = self.lines.len() - 1;
                cursor.index = self.lines.get(cursor.line)?.text().len();
                cursor_x_opt = None;
            }
            Motion::GotoLine(line) => {
                let mut layout_cursor = self.layout_cursor(font_system, cursor)?;
                layout_cursor.line = line;
                (cursor, cursor_x_opt) = self.cursor_motion(
                    font_system,
                    cursor,
                    cursor_x_opt,
                    Motion::LayoutCursor(layout_cursor),
                )?;
            }
        }
        Some((cursor, cursor_x_opt))
    }

    /// Draw the buffer
    #[cfg(feature = "swash")]
    pub fn draw<F>(
        &self,
        font_system: &mut FontSystem,
        cache: &mut crate::SwashCache,
        color: Color,
        mut f: F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        for run in self.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0., 0.), 1.0);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => color,
                };

                cache.with_pixels(
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
}

impl<'a> BorrowedWithFontSystem<'a, Buffer> {
    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(&mut self, cursor: Cursor, prune: bool) {
        self.inner
            .shape_until_cursor(self.font_system, cursor, prune);
    }

    /// Shape lines until scroll
    pub fn shape_until_scroll(&mut self, prune: bool) {
        self.inner.shape_until_scroll(self.font_system, prune);
    }

    /// Shape the provided line index and return the result
    pub fn line_shape(&mut self, line_i: usize) -> Option<&ShapeLine> {
        self.inner.line_shape(self.font_system, line_i)
    }

    /// Lay out the provided line index and return the result
    pub fn line_layout(&mut self, line_i: usize) -> Option<&[LayoutLine]> {
        self.inner.line_layout(self.font_system, line_i)
    }

    /// Set the current [`Metrics`]
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.font_size` is zero.
    pub fn set_metrics(&mut self, metrics: Metrics) {
        self.inner.set_metrics(self.font_system, metrics);
    }

    /// Set the current [`Wrap`]
    pub fn set_wrap(&mut self, wrap: Wrap) {
        self.inner.set_wrap(self.font_system, wrap);
    }

    /// Set the current buffer dimensions
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.inner.set_size(self.font_system, width, height);
    }

    /// Set the current [`Metrics`] and buffer dimensions at the same time
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.font_size` is zero.
    pub fn set_metrics_and_size(&mut self, metrics: Metrics, width: f32, height: f32) {
        self.inner
            .set_metrics_and_size(self.font_system, metrics, width, height);
    }

    /// Set text of buffer, using provided attributes for each line by default
    pub fn set_text(&mut self, text: &str, attrs: Attrs, shaping: Shaping) {
        self.inner.set_text(self.font_system, text, attrs, shaping);
    }

    /// Set text of buffer, using an iterator of styled spans (pairs of text and attributes)
    ///
    /// ```
    /// # use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
    /// # let mut font_system = FontSystem::new();
    /// let mut buffer = Buffer::new_empty(Metrics::new(32.0, 44.0));
    /// let mut buffer = buffer.borrow_with(&mut font_system);
    /// let attrs = Attrs::new().family(Family::Serif);
    /// buffer.set_rich_text(
    ///     [
    ///         ("hello, ", attrs),
    ///         ("cosmic\ntext", attrs.family(Family::Monospace)),
    ///     ],
    ///     attrs,
    ///     Shaping::Advanced,
    /// );
    /// ```
    pub fn set_rich_text<'r, 's, I>(&mut self, spans: I, default_attrs: Attrs, shaping: Shaping)
    where
        I: IntoIterator<Item = (&'s str, Attrs<'r>)>,
    {
        self.inner
            .set_rich_text(self.font_system, spans, default_attrs, shaping);
    }

    /// Apply a [`Motion`] to a [`Cursor`]
    pub fn cursor_motion(
        &mut self,
        cursor: Cursor,
        cursor_x_opt: Option<i32>,
        motion: Motion,
    ) -> Option<(Cursor, Option<i32>)> {
        self.inner
            .cursor_motion(self.font_system, cursor, cursor_x_opt, motion)
    }

    /// Draw the buffer
    #[cfg(feature = "swash")]
    pub fn draw<F>(&mut self, cache: &mut crate::SwashCache, color: Color, f: F)
    where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        self.inner.draw(self.font_system, cache, color, f);
    }
}
