// SPDX-License-Identifier: MIT OR Apache-2.0

//! This module contains the [`Buffer`] type which is the entry point for shaping and layout of text.
//!
//! A [`Buffer`] contains a list of [`BufferLine`]s and is used to compute the [`LayoutRun`]s.
//!
//! [`BufferLine`]s correspond to the paragraphs of text in the [`Buffer`].
//! Each [`BufferLine`] contains a list of [`LayoutLine`]s, which represent the visual lines.
//! `Buffer::line_heights` is a computed list of visual line heights.
//!
//! [`LayoutRun`]s represent the actually-visible visual lines,
//! based on the [`Buffer`]'s scroll position, width, height and wrapping mode.

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Affinity, Attrs, AttrsList, BidiParagraphs, BorrowedWithFontSystem, BufferLine, Color, Cursor,
    FontSystem, LayoutCursor, LayoutGlyph, LayoutLine, Motion, Scroll, ShapeBuffer, ShapeLine,
    Shaping, Wrap,
};

/// A line of visible text for rendering
#[derive(Debug)]
pub struct LayoutRun<'a> {
    /// The index of the original [`BufferLine`] (or paragraph) in the [`Buffer`]
    pub line_i: usize,
    /// The text of the original [`BufferLine`] (or paragraph)
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
    /// The height of the line
    pub line_height: f32,
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
    /// The buffer we are iterating over
    buffer: &'b Buffer,
    /// The paragraph we are up to in the iteration
    line_i: usize,
    /// The line within the paragraph we are up to in the iteration
    layout_i: usize,
    /// The total line height already iterated over
    height_seen: f32,
    /// How many items the iterator will still yield (for size hint)
    remaining_len: usize,
}

impl<'b> LayoutRunIter<'b> {
    pub fn new(buffer: &'b Buffer) -> Self {
        // Compute how many lines there will be in the iterator for the size hint. This involves iterating
        // through the entire iterator but should be relatively cheap.
        let mut remaining_height = buffer.height;
        let remaining_len: usize = buffer
            .lines
            .iter()
            .skip(buffer.scroll.line)
            .flat_map(|line| line.line_heights().unwrap_or(&[]))
            .skip(buffer.scroll.layout as usize)
            .take_while(|line_height| {
                remaining_height -= **line_height;
                remaining_height > 0.0
            })
            .count();

        Self {
            buffer,
            line_i: buffer.scroll.line,
            layout_i: buffer.scroll.layout as usize,
            height_seen: 0.0,
            remaining_len,
        }
    }
}

impl<'b> Iterator for LayoutRunIter<'b> {
    type Item = LayoutRun<'b>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining_len, Some(self.remaining_len))
    }

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_len == 0 {
            return None;
        }

        while let Some(line) = self.buffer.lines.get(self.line_i) {
            let shape = line.shape_opt().as_ref()?;
            let layout = line.layout_opt()?;
            let line_heights = line.line_heights()?;
            while let (Some(layout_line), Some(line_height)) =
                (layout.get(self.layout_i), line_heights.get(self.layout_i))
            {
                let line_top = self.height_seen;
                self.height_seen += line_height;
                self.layout_i += 1;

                let glyph_height = layout_line.max_ascent + layout_line.max_descent;
                let centering_offset = (line_height - glyph_height) / 2.0;
                let line_y = line_top + centering_offset + layout_line.max_ascent;

                if line_top + centering_offset > self.buffer.height {
                    return None;
                }

                self.remaining_len -= 1;
                return Some(LayoutRun {
                    line_i: self.line_i,
                    text: line.text(),
                    rtl: shape.rtl,
                    glyphs: &layout_line.glyphs,
                    line_y,
                    line_top,
                    line_w: layout_line.w,
                    line_height: *line_height,
                });
            }
            self.line_i += 1;
            self.layout_i = 0;
        }

        None
    }
}

impl<'b> ExactSizeIterator for LayoutRunIter<'b> {}

/// A buffer of text that is shaped and laid out
#[derive(Debug)]
pub struct Buffer {
    /// [BufferLine]s (or paragraphs) of text in the buffer.
    pub lines: Vec<BufferLine>,
    /// The cached heights of visual lines. Compute using [`Buffer::update_line_heights`].
    line_heights: Vec<f32>,
    /// The text bounding box width.
    width: f32,
    /// The text bounding box height.
    height: f32,
    scroll: Scroll,
    /// True if a redraw is requires. Set to false after processing
    redraw: bool,
    /// The wrapping mode
    wrap: Wrap,
    monospace_width: Option<f32>,

    /// Scratch buffer for shaping and laying out.
    scratch: ShapeBuffer,
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            // metrics: self.metrics,
            line_heights: self.line_heights.clone(),
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
    pub fn new_empty() -> Self {
        Self {
            lines: Vec::new(),
            line_heights: Vec::new(),
            width: 0.0,
            height: 0.0,
            scroll: Scroll::default(),
            redraw: false,
            wrap: Wrap::WordOrGlyph,
            scratch: ShapeBuffer::default(),
            monospace_width: None,
        }
    }

    /// Create a new [`Buffer`] with the provided [`FontSystem`] and [`Metrics`]
    pub fn new(font_system: &mut FontSystem) -> Self {
        let mut buffer = Self::new_empty();
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
                    // self.metrics.font_size,
                    self.width,
                    self.wrap,
                    self.monospace_width,
                );
            }
        }

        self.update_line_heights();
        self.redraw = true;

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::debug!("relayout: {:?}", instant.elapsed());
    }

    /// Get the cached heights of visual lines
    pub fn line_heights(&self) -> &[f32] {
        self.line_heights.as_slice()
    }

    /// Update the cached heights of visual lines
    pub fn update_line_heights(&mut self) {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        self.line_heights.clear();
        let iter = self
            .lines
            .iter()
            .flat_map(|line| line.line_heights())
            .flat_map(|lines| lines.iter().copied());
        self.line_heights.extend(iter);

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::debug!(
            "update_line_heights {}: {:?}",
            self.line_heights.len(),
            instant.elapsed()
        );
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

    // REMOVEME (line-heights)
    //         let mut reshaped = 0;
    //         let mut layout_i = 0;
    //         let mut should_update_line_heights = false;
    //         for (line_i, line) in self.lines.iter_mut().enumerate() {
    //             if line_i > cursor.line {
    //                 break;
    //             }

    //             if line.shape_opt().is_none() {
    //                 reshaped += 1;
    //             }

    //             if line.layout_opt().is_none() {
    //                 should_update_line_heights = true;
    //             }

    //             let layout =
    //                 line.layout_in_buffer(&mut self.scratch, font_system, self.width, self.wrap);
    //             if line_i == cursor.line {
    //                 let layout_cursor = self.layout_cursor(&cursor);
    //                 layout_i += layout_cursor.layout as i32;
    //                 break;
    //             } else {
    //                 layout_i += layout.len() as i32;
    //             }
    //         }

    //         if should_update_line_heights {
    //             self.update_line_heights();
    //         }

    //         if reshaped > 0 {
    //             #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    //             log::debug!("shape_until_cursor {}: {:?}", reshaped, instant.elapsed());
    //             self.redraw = true;
    //         }

    //         // the first visible line is index = self.scroll
    //         // the last visible line is index = self.scroll + lines
    //         let lines = self.visible_lines();
    //         if layout_i < self.scroll {
    //             self.scroll = layout_i;
    //         } else if layout_i >= self.scroll + lines {
    //             // need to work backwards from layout_i using the line heights
    //             let lines = self.visible_lines_to(layout_i as usize);
    //             self.scroll = layout_i - (lines - 1);
    //         }

    //         self.shape_until_scroll(font_system);
    //     }
    // =======

    //     /// Pre-shape lines in the buffer, up to `lines`, return actual number of layout lines
    //     pub fn shape_until(&mut self, font_system: &mut FontSystem, lines: i32) -> i32 {
    //         #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    //         let instant = std::time::Instant::now();

    //         let mut reshaped = 0;
    //         let mut total_layout = 0;
    //         let mut should_update_line_heights = false;
    //         for line in &mut self.lines {
    //             if total_layout >= lines {
    //                 break;
    //             }

    //             if line.shape_opt().is_none() {
    //                 reshaped += 1;
    //             }

    //             if line.layout_opt().is_none() {
    //                 should_update_line_heights = true;
    //             }

    //             let layout =
    //                 line.layout_in_buffer(&mut self.scratch, font_system, self.width, self.wrap);
    //             total_layout += layout.len() as i32;
    //         }

    //         if should_update_line_heights {
    //             self.update_line_heights();
    //         }

    //         if reshaped > 0 {
    //             #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    //             log::debug!("shape_until {}: {:?}", reshaped, instant.elapsed());
    //             self.redraw = true;
    //         }

    //         total_layout
    //     }

    //     /// Shape lines until scroll
    //     pub fn shape_until_scroll(&mut self, font_system: &mut FontSystem) {
    //         self.layout_lines(font_system);

    //         let lines = self.visible_lines();

    //         let scroll_end = self.scroll + lines;
    //         let total_layout = self.shape_until(font_system, scroll_end);
    //         self.scroll = (total_layout - (lines - 1)).clamp(0, self.scroll);
    //     }

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
        // REMOVEME (line-heights)
        // let line = &self.lines[cursor.line];
        // //TODO: ensure layout is done?
        // let layout = line.layout_opt().expect("layout not found");

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
        let should_update_line_heights = {
            let line = self.lines.get_mut(line_i)?;
            // check if the line needs to be laid out
            if line.layout_opt().is_none() {
                // update the layout (result will be cached)
                let _ = line.layout(font_system, self.width, self.wrap, self.monospace_width);
                true
            } else {
                false
            }
        };

        if should_update_line_heights {
            self.update_line_heights();
        }

        let line = self.lines.get_mut(line_i)?;
        Some(line.layout_in_buffer(
            &mut self.scratch,
            font_system,
            // self.metrics.font_size,
            self.width,
            self.wrap,
            self.monospace_width,
        ))
    }

    /// Lay out all lines without shaping
    pub fn layout_lines(&mut self, font_system: &mut FontSystem) {
        let mut should_update_line_heights = false;
        for line in self.lines.iter_mut() {
            if line.layout_opt().is_none() {
                should_update_line_heights = true;
                let _ = line.layout(font_system, self.width, self.wrap, self.monospace_width);
            }
        }

        if should_update_line_heights {
            self.update_line_heights();
        }
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
    pub fn set_monospace_width(
        &mut self,
        font_system: &mut FontSystem,
        monospace_width: Option<f32>,
    ) {
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
        let clamped_width = width.max(0.0);
        let clamped_height = height.max(0.0);
        if clamped_width != self.width || clamped_height != self.height {
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

    /// Get the number of lines that can be viewed in the buffer, from a starting point
    pub fn visible_lines_from(&self, from: Scroll) -> i32 {
        let mut height = self.height;
        // let line_heights = self.line_heights();
        // if line_heights.is_empty() {
        //     // this has never been laid out, so we can't know the height yet
        //     return i32::MAX;
        // }
        let mut i = 0;
        let mut para_iter = self.lines.iter().skip(from.line);

        // Iterate over first paragraph (skipping from.layout visible lines)
        let Some(first_para_line_heights) = para_iter.next().and_then(|para| para.line_heights())
        else {
            return i32::MAX;
        };
        let mut line_iter = first_para_line_heights.iter().skip(from.layout as usize);
        while let Some(line_height) = line_iter.next() {
            height -= line_height;
            if height <= 0.0 {
                break;
            }
            i += 1;
        }

        // Iterate over all lines of remaining paragraph
        let mut iter = para_iter.flat_map(|para| para.line_heights().unwrap_or(&[]).iter());
        while let Some(line_height) = iter.next() {
            height -= line_height;
            if height <= 0.0 {
                break;
            }
            i += 1;
        }

        i
    }

    /// Get the number of lines that can be viewed in the buffer, to an ending point
    pub fn visible_lines_to(&self, to: usize) -> i32 {
        let mut height = self.height;
        let line_heights = self.line_heights();
        if line_heights.is_empty() {
            // this has never been laid out, so we can't know the height yet
            return i32::MAX;
        }
        let mut i = 0;
        let mut iter = line_heights.iter().rev().skip(line_heights.len() - to - 1);
        while let Some(line_height) = iter.next() {
            height -= line_height;
            if height <= 0.0 {
                break;
            }
            i += 1;
        }
        i
    }

    /// Get the number of visual lines that can be viewed in the buffer
    pub fn visible_lines(&self) -> i32 {
        self.visible_lines_from(self.scroll)
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
    /// # use cosmic_text::{Attrs, Buffer, Family, FontSystem, LineHeight, Shaping};
    /// # let mut font_system = FontSystem::new();
    /// let mut buffer = Buffer::new_empty();
    /// let attrs = Attrs::new().size(32.0).line_height(LineHeight::Absolute(44.0)).family(Family::Serif);
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
    pub fn hit(&self, strike_x: f32, strike_y: f32) -> Option<Cursor> {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        // below,
        // - `first`, `last` refers to iterator indices (usize)
        // - `start`, `end` refers to byte indices (usize)
        // - `left`, `top`, `right`, `bot`, `mid` refers to spatial coordinates (f32)

        let Some(last_run_index) = self.layout_runs().count().checked_sub(1) else {
            return None;
        };

        let mut runs = self.layout_runs().enumerate();

        // TODO: consider caching line_top and line_bot on LayoutRun

        // 1. within the buffer, find the layout run (line) that contains the strike point
        // 2. within the layout run (line), find the glyph that contains the strike point
        // 3. within the glyph, find the approximate extended grapheme cluster (egc) that contains the strike point
        // the boundary (top/bot, left/right) cases in each step are special
        let mut line_top = 0.0;
        let cursor = 'hit: loop {
            let Some((run_index, run)) = runs.next() else {
                // no hit found
                break 'hit None;
            };

            if run_index == 0 && strike_y < line_top {
                // hit above top line
                break 'hit Some(Cursor::new(run.line_i, 0));
            }

            let line_bot = line_top + run.line_height;

            if run_index == last_run_index && strike_y >= line_bot {
                // hit below bottom line
                match run.glyphs.last() {
                    Some(glyph) => break 'hit Some(run.cursor_from_glyph_right(glyph)),
                    None => break 'hit Some(Cursor::new(run.line_i, 0)),
                }
            }

            if (line_top..line_bot).contains(&strike_y) {
                let last_glyph_index = run.glyphs.len() - 1;

                // TODO: is this assumption correct with rtl?
                let (left_glyph_index, right_glyph_index) = if run.rtl {
                    (last_glyph_index, 0)
                } else {
                    (0, last_glyph_index)
                };

                for (glyph_index, glyph) in run.glyphs.iter().enumerate() {
                    let glyph_left = glyph.x;

                    if glyph_index == left_glyph_index && strike_x < glyph_left {
                        // hit left of left-most glyph in line
                        break 'hit Some(Cursor::new(run.line_i, 0));
                    }

                    let glyph_right = glyph_left + glyph.w;

                    if glyph_index == right_glyph_index && strike_x >= glyph_right {
                        // hit right of right-most glyph in line
                        break 'hit Some(run.cursor_from_glyph_right(glyph));
                    }

                    if (glyph_left..glyph_right).contains(&strike_x) {
                        let cluster = &run.text[glyph.start..glyph.end];

                        let total = cluster.graphemes(true).count();
                        let last_egc_index = total - 1;
                        let egc_w = glyph.w / (total as f32);
                        let mut egc_left = glyph_left;

                        // TODO: is this assumption correct with rtl?
                        let (left_egc_index, right_egc_index) = if glyph.level.is_rtl() {
                            (last_egc_index, 0)
                        } else {
                            (0, last_egc_index)
                        };

                        for (egc_index, (egc_start, egc)) in
                            cluster.grapheme_indices(true).enumerate()
                        {
                            let egc_end = egc_start + egc.len();

                            let (left_egc_byte, right_egc_byte) = if glyph.level.is_rtl() {
                                (glyph.start + egc_end, glyph.start + egc_start)
                            } else {
                                (glyph.start + egc_start, glyph.start + egc_end)
                            };

                            if egc_index == left_egc_index && strike_x < egc_left {
                                // hit left of left-most egc in cluster
                                break 'hit Some(Cursor::new(run.line_i, left_egc_byte));
                            }

                            let egc_right = egc_left + egc_w;

                            if egc_index == right_egc_index && strike_x >= egc_right {
                                // hit right of right-most egc in cluster
                                break 'hit Some(Cursor::new(run.line_i, right_egc_byte));
                            }

                            let egc_mid = egc_left + egc_w / 2.0;

                            let hit_egc = if (egc_left..egc_mid).contains(&strike_x) {
                                // hit left half of egc
                                Some(true)
                            } else if (egc_mid..egc_right).contains(&strike_x) {
                                // hit right half of egc
                                Some(false)
                            } else {
                                None
                            };

                            if let Some(egc_left_half) = hit_egc {
                                break 'hit Some(Cursor::new(
                                    run.line_i,
                                    if egc_left_half {
                                        left_egc_byte
                                    } else {
                                        right_egc_byte
                                    },
                                ));
                            }

                            egc_left = egc_right;
                        }
                    }
                }
            }

            line_top = line_bot;
        };

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::trace!("click({}, {}): {:?}", strike_x, strike_y, instant.elapsed());

        cursor
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
            // REMOVEME (main)
            // Motion::Vertical(px) => {
            //     // TODO more efficient
            //     let lines = px / self.metrics().line_height as i32;
            //     match lines.cmp(&0) {
            //         cmp::Ordering::Less => {
            //             for _ in 0..-lines {
            //                 (cursor, cursor_x_opt) =
            //                     self.cursor_motion(font_system, cursor, cursor_x_opt, Motion::Up)?;
            //             }
            //         }
            //         cmp::Ordering::Greater => {
            //             for _ in 0..lines {
            //                 (cursor, cursor_x_opt) = self.cursor_motion(
            //                     font_system,
            //                     cursor,
            //                     cursor_x_opt,
            //                     Motion::Down,
            //                 )?;
            //             }
            //         }
            //         cmp::Ordering::Equal => {}
            //     }
            // }
            Motion::Vertical(mut px) => {
                // TODO more efficient
                // let cursor = self.layout_cursor(&mut font_system, cursor);
                let mut current_line = cursor.line as i32;
                let direction = px.signum();
                loop {
                    current_line += direction;
                    if current_line < 0 || current_line >= self.line_heights().len() as i32 {
                        break;
                    }

                    let current_line_height = self.line_heights()[current_line as usize];

                    match direction {
                        -1 => {
                            self.cursor_motion(font_system, cursor, cursor_x_opt, Motion::Up);
                            px -= current_line_height as i32;
                            if px >= self.size().1 as i32 {
                                break;
                            }
                        }
                        1 => {
                            self.cursor_motion(font_system, cursor, cursor_x_opt, Motion::Down);
                            px += current_line_height as i32;

                            if px <= 0 as i32 {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                // let lines = px / self.buffer.metrics().line_height as i32;
                // match lines.cmp(&0) {
                //     Ordering::Less => {
                //         for _ in 0..-lines {
                //             self.action(font_system, Action::Up);
                //         }
                //     }
                //     Ordering::Greater => {
                //         for _ in 0..lines {
                //             self.action(font_system, Action::Down);
                //         }
                //     }
                //     Ordering::Equal => {}
                // }

                // TODO: is this necessary?
                self.set_redraw(true);
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

    /// Set the current [`Wrap`]
    pub fn set_wrap(&mut self, wrap: Wrap) {
        self.inner.set_wrap(self.font_system, wrap);
    }

    /// Set the current buffer dimensions
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.inner.set_size(self.font_system, width, height);
    }

    /// Set text of buffer, using provided attributes for each line by default
    pub fn set_text(&mut self, text: &str, attrs: Attrs, shaping: Shaping) {
        self.inner.set_text(self.font_system, text, attrs, shaping);
    }

    /// Set text of buffer, using an iterator of styled spans (pairs of text and attributes)
    ///
    /// ```
    /// # use cosmic_text::{Attrs, Buffer, Family, FontSystem, LineHeight, Shaping};
    /// # let mut font_system = FontSystem::new();
    /// let mut buffer = Buffer::new_empty();
    /// let mut buffer = buffer.borrow_with(&mut font_system);
    /// let attrs = Attrs::new().size(32.0).line_height(LineHeight::Absolute(44.0)).family(Family::Serif);
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
