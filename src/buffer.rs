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
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Attrs, AttrsList, BidiParagraphs, BorrowedWithFontSystem, BufferLine, Color, FontSystem,
    LayoutGlyph, LayoutLine, ShapeBuffer, ShapeLine, Shaping, Wrap,
};

/// Current cursor location
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Cursor {
    /// Text line the cursor is on
    pub line: usize,
    /// First-byte-index of glyph at cursor (will insert behind this glyph)
    pub index: usize,
    /// Whether to associate the cursor with the run before it or the run after it if placed at the
    /// boundary between two runs
    pub affinity: Affinity,
    /// Cursor color
    pub color: Option<Color>,
}

impl Cursor {
    /// Create a new cursor
    pub const fn new(line: usize, index: usize) -> Self {
        Self::new_with_affinity(line, index, Affinity::Before)
    }

    /// Create a new cursor, specifying the affinity
    pub const fn new_with_affinity(line: usize, index: usize, affinity: Affinity) -> Self {
        Self {
            line,
            index,
            affinity,
            color: None,
        }
    }
    /// Create a new cursor, specifying the color
    pub const fn new_with_color(line: usize, index: usize, color: Color) -> Self {
        Self {
            line,
            index,
            affinity: Affinity::Before,
            color: Some(color),
        }
    }
}

/// Whether to associate cursors placed at a boundary between runs with the run before or after it.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum Affinity {
    #[default]
    Before,
    After,
}

impl Affinity {
    pub fn before(&self) -> bool {
        *self == Self::Before
    }

    pub fn after(&self) -> bool {
        *self == Self::After
    }

    pub fn from_before(before: bool) -> Self {
        if before {
            Self::Before
        } else {
            Self::After
        }
    }

    pub fn from_after(after: bool) -> Self {
        if after {
            Self::After
        } else {
            Self::Before
        }
    }
}

/// The position of a cursor within a [`Buffer`].
#[derive(Debug)]
pub struct LayoutCursor {
    pub line: usize,
    pub layout: usize,
    pub glyph: usize,
}

impl LayoutCursor {
    pub fn new(line: usize, layout: usize, glyph: usize) -> Self {
        Self {
            line,
            layout,
            glyph,
        }
    }
}

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
    buffer: &'b Buffer,
    line_i: usize,
    layout_i: usize,
    remaining_len: usize,
    total_layout: i32,
}

impl<'b> LayoutRunIter<'b> {
    pub fn new(buffer: &'b Buffer) -> Self {
        let line_heights = buffer.line_heights();
        let total_layout_lines = line_heights.len();
        let top_cropped_layout_lines =
            total_layout_lines.saturating_sub(buffer.scroll.try_into().unwrap_or_default());
        let maximum_lines: usize = buffer.visible_lines().try_into().unwrap_or_default();
        let bottom_cropped_layout_lines = if top_cropped_layout_lines > maximum_lines {
            maximum_lines
        } else {
            top_cropped_layout_lines
        };
        Self {
            buffer,
            line_i: 0,
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
            let layout = line.layout_opt()?;
            while let Some(layout_line) = layout.get(self.layout_i) {
                self.layout_i += 1;

                let scrolled = self.total_layout < self.buffer.scroll;
                self.total_layout += 1;
                if scrolled {
                    continue;
                }
                // TODO: can scroll be negative?
                let this_line = self.total_layout.saturating_sub(1) as usize;
                let line_top = self.buffer.line_heights[self.buffer.scroll as usize..this_line]
                    .iter()
                    .sum();
                let glyph_height = layout_line.max_ascent + layout_line.max_descent;
                let centering_offset = (self.buffer.line_heights[this_line] - glyph_height) / 2.0;
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
                        line_w: layout_line.width,
                        line_height: self.buffer.line_heights[this_line],
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
    /// The current scroll position in terms of visual lines.
    scroll: i32,
    /// True if a redraw is required. Set to false after processing.
    redraw: bool,
    /// The wrapping mode
    wrap: Wrap,
    /// Scratch buffer for shaping and laying out.
    scratch: ShapeBuffer,
}

impl Buffer {
    /// Create an empty [`Buffer`]
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
            scroll: 0,
            redraw: false,
            wrap: Wrap::Word,
            scratch: ShapeBuffer::default(),
        }
    }

    /// Create a new [`Buffer`] with the provided [`FontSystem`]
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
                line.layout(font_system, self.width, self.wrap);
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

    /// Pre-shape lines in the buffer, up to `lines`, return actual number of layout lines
    pub fn shape_until(&mut self, font_system: &mut FontSystem, lines: i32) -> i32 {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        let mut reshaped = 0;
        let mut total_layout = 0;
        let mut should_update_line_heights = false;
        for line in &mut self.lines {
            if total_layout >= lines {
                break;
            }

            if line.shape_opt().is_none() {
                reshaped += 1;
            }

            if line.layout_opt().is_none() {
                should_update_line_heights = true;
            }

            let layout =
                line.layout_in_buffer(&mut self.scratch, font_system, self.width, self.wrap);
            total_layout += layout.len() as i32;
        }

        if should_update_line_heights {
            self.update_line_heights();
        }

        if reshaped > 0 {
            #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
            log::debug!("shape_until {}: {:?}", reshaped, instant.elapsed());
            self.redraw = true;
        }

        total_layout
    }

    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(&mut self, font_system: &mut FontSystem, cursor: Cursor) {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        let mut reshaped = 0;
        let mut layout_i = 0;
        let mut should_update_line_heights = false;
        for (line_i, line) in self.lines.iter_mut().enumerate() {
            if line_i > cursor.line {
                break;
            }

            if line.shape_opt().is_none() {
                reshaped += 1;
            }

            if line.layout_opt().is_none() {
                should_update_line_heights = true;
            }

            let layout =
                line.layout_in_buffer(&mut self.scratch, font_system, self.width, self.wrap);
            if line_i == cursor.line {
                let layout_cursor = self.layout_cursor(&cursor);
                layout_i += layout_cursor.layout as i32;
                break;
            } else {
                layout_i += layout.len() as i32;
            }
        }

        if should_update_line_heights {
            self.update_line_heights();
        }

        if reshaped > 0 {
            #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
            log::debug!("shape_until_cursor {}: {:?}", reshaped, instant.elapsed());
            self.redraw = true;
        }

        // the first visible line is index = self.scroll
        // the last visible line is index = self.scroll + lines
        let lines = self.visible_lines();
        if layout_i < self.scroll {
            self.scroll = layout_i;
        } else if layout_i >= self.scroll + lines {
            // need to work backwards from layout_i using the line heights
            let lines = self.visible_lines_to(layout_i as usize);
            self.scroll = layout_i - (lines - 1);
        }

        self.shape_until_scroll(font_system);
    }

    /// Shape lines until scroll
    pub fn shape_until_scroll(&mut self, font_system: &mut FontSystem) {
        self.layout_lines(font_system);

        let lines = self.visible_lines();

        let scroll_end = self.scroll + lines;
        let total_layout = self.shape_until(font_system, scroll_end);
        self.scroll = (total_layout - lines).clamp(0, self.scroll);
    }

    pub fn layout_cursor(&self, cursor: &Cursor) -> LayoutCursor {
        let line = &self.lines[cursor.line];

        //TODO: ensure layout is done?
        let layout = line.layout_opt().expect("layout not found");
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
                if *cursor == cursor_left {
                    return LayoutCursor::new(cursor.line, layout_i, glyph_i);
                }
                if *cursor == cursor_right {
                    return LayoutCursor::new(cursor.line, layout_i, glyph_i + 1);
                }
            }
        }

        // Fall back to start of line
        //TODO: should this be the end of the line?
        LayoutCursor::new(cursor.line, 0, 0)
    }

    /// Shape the provided line index and return the result
    pub fn line_shape(
        &mut self,
        font_system: &mut FontSystem,
        line_i: usize,
    ) -> Option<&ShapeLine> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.shape(font_system))
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
                let _ = line.layout(font_system, self.width, self.wrap);
                true
            } else {
                false
            }
        };

        if should_update_line_heights {
            self.update_line_heights();
        }

        let line = self.lines.get_mut(line_i)?;

        // return cached layout
        Some(line.layout(font_system, self.width, self.wrap))
    }

    /// Lay out all lines without shaping
    pub fn layout_lines(&mut self, font_system: &mut FontSystem) {
        let mut should_update_line_heights = false;
        for line in self.lines.iter_mut() {
            if line.layout_opt().is_none() {
                should_update_line_heights = true;
                let _ = line.layout(font_system, self.width, self.wrap);
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
            self.shape_until_scroll(font_system);
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
        self.width = clamped_width;
        self.height = clamped_height;
        self.relayout(font_system);
        self.shape_until_scroll(font_system);
    }

    /// Get the current scroll location in terms of visual lines
    pub fn scroll(&self) -> i32 {
        self.scroll
    }

    /// Set the current scroll location in terms of visual lines.
    ///
    /// This is clamped to the visual lines of the buffer.
    pub fn set_scroll(&mut self, scroll: i32) {
        let visual_lines = self.line_heights().len() as i32;
        let scroll = scroll.clamp(0, visual_lines - 1);
        if scroll != self.scroll {
            self.scroll = scroll;
            self.redraw = true;
        }
    }

    /// Get the number of lines that can be viewed in the buffer, from a starting point
    pub fn visible_lines_from(&self, from: usize) -> i32 {
        let mut height = self.height;
        let line_heights = self.line_heights();
        if line_heights.is_empty() {
            // this has never been laid out, so we can't know the height yet
            return i32::MAX;
        }
        let mut i = 0;
        let mut iter = line_heights.iter().skip(from);
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
        self.visible_lines_from(self.scroll as usize)
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
    /// # use cosmic_text::{Attrs, Buffer, Family, FontSystem, Shaping};
    /// # let mut font_system = FontSystem::new();
    /// let mut buffer = Buffer::new_empty();
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

                    let attr_list = match &maybe_span {
                        // there can be newlines, that has their own span attributes, "\n" lines for example, thus add
                        // their spans if they need it
                        Some((attr, range)) => {
                            let mut list = AttrsList::new(default_attrs);
                            if *attr != attrs_list.defaults() {
                                list.add_span(range.clone(), attr.to_owned());
                            }
                            list
                        }
                        None => AttrsList::new(default_attrs),
                    };
                    let prev_attrs_list = core::mem::replace(&mut attrs_list, attr_list);
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

        self.scroll = 0;

        self.shape_until_scroll(font_system);
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
    /// Pre-shape lines in the buffer, up to `lines`, return actual number of layout lines
    pub fn shape_until(&mut self, lines: i32) -> i32 {
        self.inner.shape_until(self.font_system, lines)
    }

    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(&mut self, cursor: Cursor) {
        self.inner.shape_until_cursor(self.font_system, cursor);
    }

    /// Shape lines until scroll
    pub fn shape_until_scroll(&mut self) {
        self.inner.shape_until_scroll(self.font_system);
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
    /// # use cosmic_text::{Attrs, Buffer, Family, FontSystem, Shaping};
    /// # let mut font_system = FontSystem::new();
    /// let mut buffer = Buffer::new_empty();
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

    /// Draw the buffer
    #[cfg(feature = "swash")]
    pub fn draw<F>(&mut self, cache: &mut crate::SwashCache, color: Color, f: F)
    where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        self.inner.draw(self.font_system, cache, color, f);
    }
}
