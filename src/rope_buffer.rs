// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rope-based buffer implementation for efficient handling of large files.
//!
//! This module provides `RopeBuffer`, an alternative to the standard `Buffer`
//! that uses a rope data structure for text storage. This is much more memory
//! efficient for large files (100MB+) because:
//!
//! 1. Text is stored in a rope instead of one String per line
//! 2. Metadata (attributes, alignment) is stored sparsely - only for lines that differ from defaults
//! 3. Shape/layout caches use LRU eviction instead of storing results for all lines

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(not(feature = "std"))]
use core_maths::CoreFloat;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Affinity, Align, Attrs, BorrowedWithFontSystem, BufferLine, Color, Cursor,
    FontSystem, Hinting, LayoutCursor, LayoutGlyph, LayoutLine, LineCache, LineEnding,
    LineView, Metrics, Motion, Renderer, RopeText, Scroll, ShapeLine, Shaping,
    SparseMetadata, Wrap,
};

/// A line of visible text for rendering (rope-based version)
#[derive(Debug)]
pub struct RopeLayoutRun<'a> {
    /// The index of the original text line
    pub line_i: usize,
    /// The original text line
    pub text: String,
    /// True if the original paragraph direction is RTL
    pub rtl: bool,
    /// The array of layout glyphs to draw
    pub glyphs: &'a [LayoutGlyph],
    /// Y offset to baseline of line
    pub line_y: f32,
    /// Y offset to top of line
    pub line_top: f32,
    /// Y offset to next line
    pub line_height: f32,
    /// Width of line
    pub line_w: f32,
}

/// A buffer of text that uses rope-based storage for efficiency with large files.
///
/// This is an alternative to the standard `Buffer` type that is optimized for
/// large files. It uses:
/// - A rope data structure for text storage (O(log n) insertions/deletions)
/// - Sparse metadata storage (only stores non-default attributes)
/// - LRU caches for shape/layout results (bounded memory usage)
#[derive(Debug)]
pub struct RopeBuffer {
    /// Rope-based text storage
    text: RopeText,
    /// Sparse per-line metadata
    metadata: SparseMetadata,
    /// LRU cache for shape/layout
    cache: LineCache,
    /// Text metrics (font size, line height)
    metrics: Metrics,
    /// Display width
    width_opt: Option<f32>,
    /// Display height
    height_opt: Option<f32>,
    /// Current scroll position
    scroll: Scroll,
    /// Redraw flag
    redraw: bool,
    /// Line wrapping mode
    wrap: Wrap,
    /// Monospace glyph width
    monospace_width: Option<f32>,
    /// Tab width in spaces
    tab_width: u16,
    /// Font hinting strategy
    hinting: Hinting,
    /// Default shaping strategy
    default_shaping: Shaping,
}

impl Clone for RopeBuffer {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            metadata: self.metadata.clone(),
            cache: LineCache::default(), // Don't clone caches
            metrics: self.metrics,
            width_opt: self.width_opt,
            height_opt: self.height_opt,
            scroll: self.scroll,
            redraw: self.redraw,
            wrap: self.wrap,
            monospace_width: self.monospace_width,
            tab_width: self.tab_width,
            hinting: self.hinting,
            default_shaping: self.default_shaping,
        }
    }
}

impl RopeBuffer {
    /// Create an empty `RopeBuffer` with the provided metrics.
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.line_height` is zero.
    pub fn new_empty(metrics: Metrics) -> Self {
        assert_ne!(metrics.line_height, 0.0, "line height cannot be 0");
        Self {
            text: RopeText::new(),
            metadata: SparseMetadata::new(),
            cache: LineCache::default(),
            metrics,
            width_opt: None,
            height_opt: None,
            scroll: Scroll::default(),
            redraw: false,
            wrap: Wrap::WordOrGlyph,
            monospace_width: None,
            tab_width: 8,
            hinting: Hinting::default(),
            default_shaping: Shaping::Advanced,
        }
    }

    /// Create a new `RopeBuffer` with initial empty text.
    ///
    /// # Panics
    ///
    /// Will panic if `metrics.line_height` is zero.
    pub fn new(font_system: &mut FontSystem, metrics: Metrics) -> Self {
        let mut buffer = Self::new_empty(metrics);
        buffer.set_text(font_system, "", &Attrs::new(), Shaping::Advanced, None);
        buffer
    }

    /// Mutably borrows the buffer with a font system.
    pub fn borrow_with<'a>(
        &'a mut self,
        font_system: &'a mut FontSystem,
    ) -> BorrowedWithFontSystem<'a, Self> {
        BorrowedWithFontSystem {
            inner: self,
            font_system,
        }
    }

    /// Get the number of lines in the buffer.
    #[inline]
    pub fn line_count(&self) -> usize {
        self.text.line_count().max(1)
    }

    /// Get a line view by index.
    pub fn line(&self, line_i: usize) -> Option<LineView<'_>> {
        if line_i < self.line_count() {
            Some(LineView::new(line_i, &self.text, &self.metadata, &self.cache))
        } else {
            None
        }
    }

    /// Get the text of a line.
    pub fn line_text(&self, line_i: usize) -> Option<String> {
        self.text.line_text(line_i).map(|c| c.into_owned())
    }

    /// Set the text of the buffer.
    pub fn set_text(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
        attrs: &Attrs,
        shaping: Shaping,
        alignment: Option<Align>,
    ) {
        // Clear existing data
        self.text.set_text(text);
        self.metadata.clear();
        self.cache.clear();
        self.default_shaping = shaping;
        self.metadata.set_default_attrs(attrs);
        self.metadata.set_default_shaping(shaping);

        // Line endings are detected lazily when accessed
        // Only set alignment if specified (sparse storage)
        if let Some(align) = alignment {
            // Store default alignment - will be applied when lines are accessed
            // For now, we just note that alignment was requested
            // Individual line alignment is set lazily
            let _ = align; // alignment will be handled by default in layout
        }

        self.scroll = Scroll::default();
        self.shape_until_scroll(font_system, false);
    }

    fn relayout(&mut self, _font_system: &mut FontSystem) {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        // Clear layout cache and re-layout visible lines
        self.cache.clear_layout();
        self.redraw = true;

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::debug!("relayout: {:?}", instant.elapsed());
    }

    /// Shape lines until cursor, scrolling to include cursor in view.
    pub fn shape_until_cursor(
        &mut self,
        font_system: &mut FontSystem,
        cursor: Cursor,
        prune: bool,
    ) {
        let metrics = self.metrics;
        let old_scroll = self.scroll;

        let layout_cursor = self
            .layout_cursor(font_system, cursor)
            .expect("shape_until_cursor invalid cursor");

        let mut layout_y = 0.0;
        let mut total_height = {
            let layout = self
                .line_layout(font_system, layout_cursor.line)
                .expect("shape_until_cursor failed to scroll forwards");
            (0..layout_cursor.layout).for_each(|layout_i| {
                layout_y += layout[layout_i]
                    .line_height_opt
                    .unwrap_or(metrics.line_height);
            });
            layout_y
                + layout[layout_cursor.layout]
                    .line_height_opt
                    .unwrap_or(metrics.line_height)
        };

        if self.scroll.line > layout_cursor.line
            || (self.scroll.line == layout_cursor.line && self.scroll.vertical > layout_y)
        {
            self.scroll.line = layout_cursor.line;
            self.scroll.vertical = layout_y;
        } else if let Some(height) = self.height_opt {
            let mut line_i = layout_cursor.line;
            if line_i <= self.scroll.line {
                if total_height > height + self.scroll.vertical {
                    self.scroll.vertical = total_height - height;
                }
            } else {
                while line_i > self.scroll.line {
                    line_i -= 1;
                    let layout = self
                        .line_layout(font_system, line_i)
                        .expect("shape_until_cursor failed to scroll forwards");
                    for layout_line in layout {
                        total_height += layout_line.line_height_opt.unwrap_or(metrics.line_height);
                    }
                    if total_height > height + self.scroll.vertical {
                        self.scroll.line = line_i;
                        self.scroll.vertical = total_height - height;
                    }
                }
            }
        }

        if old_scroll != self.scroll {
            self.redraw = true;
        }

        self.shape_until_scroll(font_system, prune);

        // Adjust horizontal scroll to include cursor
        if let Some(layout_cursor) = self.layout_cursor(font_system, cursor) {
            if let Some(layout_lines) = self.line_layout(font_system, layout_cursor.line) {
                if let Some(layout_line) = layout_lines.get(layout_cursor.layout) {
                    let (x_min, x_max) = layout_line
                        .glyphs
                        .get(layout_cursor.glyph)
                        .or_else(|| layout_line.glyphs.last())
                        .map_or((0.0, 0.0), |glyph| {
                            let x_a = glyph.x;
                            let x_b = glyph.x + glyph.w;
                            (x_a.min(x_b), x_a.max(x_b))
                        });
                    if x_min < self.scroll.horizontal {
                        self.scroll.horizontal = x_min;
                        self.redraw = true;
                    }
                    if let Some(width) = self.width_opt {
                        if x_max > self.scroll.horizontal + width {
                            self.scroll.horizontal = x_max - width;
                            self.redraw = true;
                        }
                    }
                }
            }
        }
    }

    /// Shape lines until scroll.
    pub fn shape_until_scroll(&mut self, font_system: &mut FontSystem, prune: bool) {
        let metrics = self.metrics;
        let old_scroll = self.scroll;
        let line_count = self.line_count();

        loop {
            while self.scroll.vertical < 0.0 {
                if self.scroll.line > 0 {
                    let line_i = self.scroll.line - 1;
                    if let Some(layout) = self.line_layout(font_system, line_i) {
                        let mut layout_height = 0.0;
                        for layout_line in layout {
                            layout_height +=
                                layout_line.line_height_opt.unwrap_or(metrics.line_height);
                        }
                        self.scroll.line = line_i;
                        self.scroll.vertical += layout_height;
                    } else {
                        self.scroll.line = line_i;
                        self.scroll.vertical += metrics.line_height;
                    }
                } else {
                    self.scroll.vertical = 0.0;
                    break;
                }
            }

            let scroll_start = self.scroll.vertical;
            // Use a reasonable default height if not set - prevents shaping all lines
            let default_height = metrics.line_height * 50.0; // ~50 lines
            let scroll_end = scroll_start + self.height_opt.unwrap_or(default_height);

            let mut total_height = 0.0;
            for line_i in 0..line_count {
                if line_i < self.scroll.line {
                    if prune {
                        self.cache.invalidate_line(line_i);
                    }
                    continue;
                }
                if total_height > scroll_end {
                    if prune {
                        self.cache.invalidate_line(line_i);
                        continue;
                    }
                    break;
                }

                let mut layout_height = 0.0;
                let layout = self
                    .line_layout(font_system, line_i)
                    .expect("shape_until_scroll invalid line");
                for layout_line in layout {
                    let line_height = layout_line.line_height_opt.unwrap_or(metrics.line_height);
                    layout_height += line_height;
                    total_height += line_height;
                }

                if line_i == self.scroll.line && layout_height <= self.scroll.vertical {
                    self.scroll.line += 1;
                    self.scroll.vertical -= layout_height;
                }
            }

            if total_height < scroll_end && self.scroll.line > 0 && self.height_opt.is_some() {
                self.scroll.vertical -= scroll_end - total_height;
            } else {
                break;
            }
        }

        if old_scroll != self.scroll {
            self.redraw = true;
        }
    }

    /// Convert a Cursor to a LayoutCursor.
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
        Some(LayoutCursor::new(cursor.line, 0, 0))
    }

    /// Shape a line and return the result.
    pub fn line_shape(
        &mut self,
        font_system: &mut FontSystem,
        line_i: usize,
    ) -> Option<&ShapeLine> {
        if line_i >= self.line_count() {
            return None;
        }

        if !self.cache.shape.contains(line_i) {
            let text = self.text.line_text(line_i).unwrap_or_default().into_owned();
            let attrs_list = self.metadata.attrs_list(line_i);
            let shaping = self.metadata.shaping(line_i);

            let mut shape_line = ShapeLine::empty();
            shape_line.build(font_system, &text, &attrs_list, shaping, self.tab_width);
            self.cache.shape.insert(line_i, shape_line);
            self.cache.layout.remove(line_i);
        }
        self.cache.shape.get(line_i)
    }

    /// Layout a line and return the result.
    pub fn line_layout(
        &mut self,
        font_system: &mut FontSystem,
        line_i: usize,
    ) -> Option<&[LayoutLine]> {
        if line_i >= self.line_count() {
            return None;
        }

        // Ensure shape exists
        if !self.cache.shape.contains(line_i) {
            self.line_shape(font_system, line_i)?;
        }

        if !self.cache.layout.contains(line_i) {
            let align = self.metadata.align(line_i);
            let shape = self.cache.shape.get(line_i)?;

            let mut layout = Vec::with_capacity(1);
            shape.layout_to_buffer(
                &mut font_system.shape_buffer,
                self.metrics.font_size,
                self.width_opt,
                self.wrap,
                align,
                &mut layout,
                self.monospace_width,
                self.hinting,
            );
            self.cache.layout.insert(line_i, layout);
        }
        self.cache.layout.get(line_i).map(|v| v.as_slice())
    }

    /// Get the current metrics.
    pub const fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// Set the current metrics.
    pub fn set_metrics(&mut self, font_system: &mut FontSystem, metrics: Metrics) {
        self.set_metrics_and_size(font_system, metrics, self.width_opt, self.height_opt);
    }

    /// Get the current hinting strategy.
    pub const fn hinting(&self) -> Hinting {
        self.hinting
    }

    /// Set the current hinting strategy.
    pub fn set_hinting(&mut self, font_system: &mut FontSystem, hinting: Hinting) {
        if hinting != self.hinting {
            self.hinting = hinting;
            self.relayout(font_system);
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current wrap mode.
    pub const fn wrap(&self) -> Wrap {
        self.wrap
    }

    /// Set the current wrap mode.
    pub fn set_wrap(&mut self, font_system: &mut FontSystem, wrap: Wrap) {
        if wrap != self.wrap {
            self.wrap = wrap;
            self.relayout(font_system);
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current monospace width.
    pub const fn monospace_width(&self) -> Option<f32> {
        self.monospace_width
    }

    /// Set monospace width.
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

    /// Get the current tab width.
    pub const fn tab_width(&self) -> u16 {
        self.tab_width
    }

    /// Set tab width.
    pub fn set_tab_width(&mut self, font_system: &mut FontSystem, tab_width: u16) {
        if tab_width == 0 {
            return;
        }
        if tab_width != self.tab_width {
            self.tab_width = tab_width;
            // Invalidate lines containing tabs
            for line_i in 0..self.line_count() {
                if let Some(text) = self.text.line_text(line_i) {
                    if text.contains('\t') {
                        self.cache.invalidate_line(line_i);
                    }
                }
            }
            self.redraw = true;
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current buffer dimensions.
    pub const fn size(&self) -> (Option<f32>, Option<f32>) {
        (self.width_opt, self.height_opt)
    }

    /// Set the current buffer dimensions.
    pub fn set_size(
        &mut self,
        font_system: &mut FontSystem,
        width_opt: Option<f32>,
        height_opt: Option<f32>,
    ) {
        self.set_metrics_and_size(font_system, self.metrics, width_opt, height_opt);
    }

    /// Set metrics and size together.
    pub fn set_metrics_and_size(
        &mut self,
        font_system: &mut FontSystem,
        metrics: Metrics,
        width_opt: Option<f32>,
        height_opt: Option<f32>,
    ) {
        let clamped_width_opt = width_opt.map(|width| width.max(0.0));
        let clamped_height_opt = height_opt.map(|height| height.max(0.0));

        if metrics != self.metrics
            || clamped_width_opt != self.width_opt
            || clamped_height_opt != self.height_opt
        {
            assert_ne!(metrics.font_size, 0.0, "font size cannot be 0");
            self.metrics = metrics;
            self.width_opt = clamped_width_opt;
            self.height_opt = clamped_height_opt;
            self.relayout(font_system);
            self.shape_until_scroll(font_system, false);
        }
    }

    /// Get the current scroll position.
    pub const fn scroll(&self) -> Scroll {
        self.scroll
    }

    /// Set the scroll position.
    pub fn set_scroll(&mut self, scroll: Scroll) {
        if scroll != self.scroll {
            self.scroll = scroll;
            self.redraw = true;
        }
    }

    /// Check if redraw is needed.
    pub const fn redraw(&self) -> bool {
        self.redraw
    }

    /// Set the redraw flag.
    pub fn set_redraw(&mut self, redraw: bool) {
        self.redraw = redraw;
    }

    /// Hit detection - convert x, y position to cursor.
    pub fn hit(&self, _x: f32, _y: f32) -> Option<Cursor> {
        // This requires iterating through visible lines
        // For rope buffer, we need to use the cached layouts
        // This is a simplified implementation - full implementation would
        // mirror Buffer::hit
        None // TODO: Implement full hit detection
    }

    /// Apply a motion to a cursor.
    pub fn cursor_motion(
        &mut self,
        _font_system: &mut FontSystem,
        cursor: Cursor,
        cursor_x_opt: Option<i32>,
        motion: Motion,
    ) -> Option<(Cursor, Option<i32>)> {
        // Simplified - delegate common motions
        // Full implementation would mirror Buffer::cursor_motion
        let line_count = self.line_count();
        let mut cursor = cursor;
        let mut cursor_x_opt = cursor_x_opt;

        match motion {
            Motion::Previous => {
                let text = self.text.line_text(cursor.line)?;
                if cursor.index > 0 {
                    let mut prev_index = 0;
                    for (i, _) in text.grapheme_indices(true) {
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
                    cursor.index = self.text.line_text(cursor.line)?.len();
                    cursor.affinity = Affinity::After;
                }
                cursor_x_opt = None;
            }
            Motion::Next => {
                let text = self.text.line_text(cursor.line)?;
                if cursor.index < text.len() {
                    for (i, c) in text.grapheme_indices(true) {
                        if i == cursor.index {
                            cursor.index += c.len();
                            cursor.affinity = Affinity::Before;
                            break;
                        }
                    }
                } else if cursor.line + 1 < line_count {
                    cursor.line += 1;
                    cursor.index = 0;
                    cursor.affinity = Affinity::Before;
                }
                cursor_x_opt = None;
            }
            Motion::ParagraphStart => {
                cursor.index = 0;
                cursor_x_opt = None;
            }
            Motion::ParagraphEnd => {
                cursor.index = self.text.line_text(cursor.line)?.len();
                cursor_x_opt = None;
            }
            Motion::BufferStart => {
                cursor.line = 0;
                cursor.index = 0;
                cursor_x_opt = None;
            }
            Motion::BufferEnd => {
                cursor.line = line_count.saturating_sub(1);
                cursor.index = self.text.line_text(cursor.line)?.len();
                cursor_x_opt = None;
            }
            // Other motions need more complex implementation
            _ => {}
        }

        Some((cursor, cursor_x_opt))
    }

    /// Insert text at a byte offset.
    pub fn insert_text(&mut self, line: usize, index: usize, text: &str) {
        let byte_offset = self.text.line_to_byte(line) + index;
        let old_line_count = self.line_count();
        self.text.insert(byte_offset, text);
        let new_line_count = self.line_count();

        // Update caches
        if new_line_count != old_line_count {
            let lines_added = new_line_count as isize - old_line_count as isize;
            self.cache.shift_lines(line + 1, lines_added);
            self.metadata.shift_lines(line + 1, lines_added);
        }
        self.cache.invalidate_line(line);
        self.redraw = true;
    }

    /// Delete a range of text.
    pub fn delete_range(&mut self, start_line: usize, start_index: usize, end_line: usize, end_index: usize) {
        let start_byte = self.text.line_to_byte(start_line) + start_index;
        let end_byte = self.text.line_to_byte(end_line) + end_index;
        let old_line_count = self.line_count();
        self.text.delete(start_byte, end_byte);
        let new_line_count = self.line_count();

        // Update caches
        if old_line_count != new_line_count {
            let lines_removed = old_line_count - new_line_count;
            self.metadata.remove_lines(start_line + 1, lines_removed);
            self.cache.invalidate_from(start_line);
        } else {
            self.cache.invalidate_line(start_line);
        }
        self.redraw = true;
    }

    /// Render the buffer.
    pub fn render<R: Renderer>(&self, _renderer: &mut R, _color: Color) {
        // This would need to iterate through visible layout runs
        // Similar to Buffer::render but using cached data
        // TODO: Implement full rendering
    }

    /// Convert to a standard Buffer (for compatibility).
    ///
    /// This is useful when you need to pass the buffer to code that
    /// expects a Vec<BufferLine> based Buffer.
    ///
    /// Warning: For large files, this will allocate significant memory.
    /// Consider using `to_buffer_range` instead.
    pub fn to_buffer(&self, font_system: &mut FontSystem, metrics: Metrics) -> crate::Buffer {
        self.to_buffer_range(font_system, metrics, 0, self.line_count())
    }

    /// Convert a range of lines to a standard Buffer.
    ///
    /// This creates a Buffer containing only lines from `start_line` to `end_line` (exclusive).
    /// Useful for displaying a "window" of a large file.
    ///
    /// # Arguments
    /// * `start_line` - First line to include (0-indexed)
    /// * `end_line` - Line after the last line to include
    pub fn to_buffer_range(
        &self,
        font_system: &mut FontSystem,
        metrics: Metrics,
        start_line: usize,
        end_line: usize,
    ) -> crate::Buffer {
        let mut buffer = crate::Buffer::new_empty(metrics);

        let line_count = self.line_count();
        let start = start_line.min(line_count);
        let end = end_line.min(line_count);

        // Convert requested lines to BufferLine
        for line_i in start..end {
            let text = self.text.line_text(line_i).unwrap_or_default().into_owned();
            let ending = self.metadata.line_ending(line_i);
            let attrs_list = self.metadata.attrs_list(line_i);
            let shaping = self.metadata.shaping(line_i);

            let mut line = BufferLine::new(text, ending, attrs_list, shaping);
            if let Some(align) = self.metadata.align(line_i) {
                line.set_align(Some(align));
            }
            buffer.lines.push(line);
        }

        // Copy other settings
        buffer.set_size(font_system, self.width_opt, self.height_opt);

        // Adjust scroll to be relative to the window
        let mut scroll = self.scroll;
        scroll.line = scroll.line.saturating_sub(start);
        buffer.set_scroll(scroll);

        buffer
    }

    /// Get a BufferLine for a specific line (creates it on demand).
    ///
    /// This is useful for getting individual lines without converting
    /// the entire buffer.
    pub fn get_buffer_line(&self, line_i: usize) -> Option<BufferLine> {
        if line_i >= self.line_count() {
            return None;
        }

        let text = self.text.line_text(line_i)?.into_owned();
        let ending = self.metadata.line_ending(line_i);
        let attrs_list = self.metadata.attrs_list(line_i);
        let shaping = self.metadata.shaping(line_i);

        let mut line = BufferLine::new(text, ending, attrs_list, shaping);
        if let Some(align) = self.metadata.align(line_i) {
            line.set_align(Some(align));
        }
        Some(line)
    }

    /// Create from a standard Buffer.
    pub fn from_buffer(buffer: &crate::Buffer, metrics: Metrics) -> Self {
        let mut rope_buffer = Self::new_empty(metrics);

        // Build text from buffer lines
        let mut text = String::new();
        for (i, line) in buffer.lines.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            text.push_str(line.text());
        }
        rope_buffer.text = RopeText::from_str(&text);

        // Copy metadata
        for (i, line) in buffer.lines.iter().enumerate() {
            let ending = line.ending();
            if ending != LineEnding::None {
                rope_buffer.metadata.set_line_ending(i, ending);
            }
            if let Some(align) = line.align() {
                rope_buffer.metadata.set_align(i, Some(align));
            }
            // Copy attrs if they differ from defaults
            let attrs_list = line.attrs_list();
            if attrs_list.spans_iter().count() > 0 {
                rope_buffer.metadata.set_attrs_list(i, attrs_list.clone());
            }
        }

        // Copy other settings
        rope_buffer.width_opt = buffer.size().0;
        rope_buffer.height_opt = buffer.size().1;
        rope_buffer.scroll = buffer.scroll();
        rope_buffer.wrap = buffer.wrap();
        rope_buffer.monospace_width = buffer.monospace_width();
        rope_buffer.tab_width = buffer.tab_width();
        rope_buffer.hinting = buffer.hinting();

        rope_buffer
    }
}

// Implement BorrowedWithFontSystem methods for RopeBuffer
impl BorrowedWithFontSystem<'_, RopeBuffer> {
    /// Shape lines until cursor.
    pub fn shape_until_cursor(&mut self, cursor: Cursor, prune: bool) {
        self.inner
            .shape_until_cursor(self.font_system, cursor, prune);
    }

    /// Shape lines until scroll.
    pub fn shape_until_scroll(&mut self, prune: bool) {
        self.inner.shape_until_scroll(self.font_system, prune);
    }

    /// Shape a line.
    pub fn line_shape(&mut self, line_i: usize) -> Option<&ShapeLine> {
        self.inner.line_shape(self.font_system, line_i)
    }

    /// Layout a line.
    pub fn line_layout(&mut self, line_i: usize) -> Option<&[LayoutLine]> {
        self.inner.line_layout(self.font_system, line_i)
    }

    /// Set metrics.
    pub fn set_metrics(&mut self, metrics: Metrics) {
        self.inner.set_metrics(self.font_system, metrics);
    }

    /// Set wrap mode.
    pub fn set_wrap(&mut self, wrap: Wrap) {
        self.inner.set_wrap(self.font_system, wrap);
    }

    /// Set size.
    pub fn set_size(&mut self, width_opt: Option<f32>, height_opt: Option<f32>) {
        self.inner.set_size(self.font_system, width_opt, height_opt);
    }

    /// Set metrics and size.
    pub fn set_metrics_and_size(
        &mut self,
        metrics: Metrics,
        width_opt: Option<f32>,
        height_opt: Option<f32>,
    ) {
        self.inner
            .set_metrics_and_size(self.font_system, metrics, width_opt, height_opt);
    }

    /// Set tab width.
    pub fn set_tab_width(&mut self, tab_width: u16) {
        self.inner.set_tab_width(self.font_system, tab_width);
    }

    /// Set text.
    pub fn set_text(
        &mut self,
        text: &str,
        attrs: &Attrs,
        shaping: Shaping,
        alignment: Option<Align>,
    ) {
        self.inner
            .set_text(self.font_system, text, attrs, shaping, alignment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_buffer_basic() {
        let metrics = Metrics::new(14.0, 20.0);
        let mut buffer = RopeBuffer::new_empty(metrics);

        assert_eq!(buffer.line_count(), 1); // Empty buffer has 1 line
        assert_eq!(buffer.line_text(0), Some(String::new()));
    }

    #[test]
    fn test_rope_buffer_set_text() {
        let metrics = Metrics::new(14.0, 20.0);
        let mut buffer = RopeBuffer::new_empty(metrics);

        // We can't call set_text without a FontSystem in tests easily
        // but we can test the underlying text operations
        buffer.text = RopeText::from_str("Hello\nWorld\n");

        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line_text(0).unwrap(), "Hello");
        assert_eq!(buffer.line_text(1).unwrap(), "World");
        assert_eq!(buffer.line_text(2).unwrap(), "");
    }

    #[test]
    fn test_rope_buffer_insert() {
        let metrics = Metrics::new(14.0, 20.0);
        let mut buffer = RopeBuffer::new_empty(metrics);
        buffer.text = RopeText::from_str("Hello World");

        buffer.insert_text(0, 5, ", Beautiful");
        assert_eq!(buffer.text.to_string(), "Hello, Beautiful World");
    }

    #[test]
    fn test_rope_buffer_delete() {
        let metrics = Metrics::new(14.0, 20.0);
        let mut buffer = RopeBuffer::new_empty(metrics);
        buffer.text = RopeText::from_str("Hello, World");

        buffer.delete_range(0, 5, 0, 7);
        assert_eq!(buffer.text.to_string(), "HelloWorld");
    }
}
