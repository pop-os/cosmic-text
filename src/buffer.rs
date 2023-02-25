// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{cmp, fmt};
use unicode_segmentation::UnicodeSegmentation;

#[cfg(feature = "swash")]
use crate::Color;
use crate::{
    Attrs, AttrsList, BufferLine, Ellipsize, FontSystem, HeightLimit, LayoutGlyph, LayoutLine,
    ShapeLine, Wrap,
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
        }
    }
}

/// Whether to associate cursors placed at a boundary between runs with the run before or after it.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Affinity {
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

impl Default for Affinity {
    fn default() -> Self {
        Affinity::Before
    }
}

/// The position of a cursor within a [`Buffer`].
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
    /// width of line
    pub line_w: f32,
}

impl<'a> LayoutRun<'a> {
    /// Return the pixel span Some((x_left, x_width)) of the highlighted area between cursor_start
    /// and cursor_end within this run, or None if the cursor range does not intersect this run.
    /// This may return widths of zero if cursor_start == cursor_end, if the run is empty, or if the
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
pub struct LayoutRunIter<'a, 'b> {
    buffer: &'b Buffer<'a>,
    line_i: usize,
    layout_i: usize,
    remaining_len: usize,
    line_y: i32,
    total_layout: i32,
}

impl<'a, 'b> LayoutRunIter<'a, 'b> {
    pub fn new(buffer: &'b Buffer<'a>) -> Self {
        let total_layout_lines: usize = buffer
            .lines
            .iter()
            .map(|line| {
                line.layout_opt()
                    .as_ref()
                    .map(|layout| layout.len())
                    .unwrap_or_default()
            })
            .sum();
        let top_cropped_layout_lines =
            total_layout_lines.saturating_sub(buffer.scroll.try_into().unwrap_or_default());
        let maximum_lines = buffer
            .height
            .checked_div(buffer.metrics.line_height)
            .unwrap_or_default();
        let bottom_cropped_layout_lines =
            if top_cropped_layout_lines > maximum_lines.try_into().unwrap_or_default() {
                maximum_lines.try_into().unwrap_or_default()
            } else {
                top_cropped_layout_lines
            };
        Self {
            buffer,
            line_i: 0,
            layout_i: 0,
            remaining_len: bottom_cropped_layout_lines as usize,
            line_y: buffer.metrics.y_offset(),
            total_layout: 0,
        }
    }
}

impl<'a, 'b> Iterator for LayoutRunIter<'a, 'b> {
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

                let scrolled = self.total_layout < self.buffer.scroll;
                self.total_layout += 1;
                if scrolled {
                    continue;
                }

                self.line_y += self.buffer.metrics.line_height;
                if self.line_y - self.buffer.metrics.y_offset() > self.buffer.height {
                    return None;
                }

                self.remaining_len -= 1;
                return Some(LayoutRun {
                    line_i: self.line_i,
                    text: line.text(),
                    rtl: shape.rtl,
                    glyphs: &layout_line.glyphs,
                    line_y: self.line_y,
                    line_w: layout_line.w,
                });
            }
            self.line_i += 1;
            self.layout_i = 0;
        }

        None
    }
}

impl<'a, 'b> ExactSizeIterator for LayoutRunIter<'a, 'b> {}

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
        Self {
            font_size,
            line_height,
        }
    }

    pub const fn scale(self, scale: i32) -> Self {
        Self {
            font_size: self.font_size * scale,
            line_height: self.line_height * scale,
        }
    }

    fn y_offset(&self) -> i32 {
        self.font_size - self.line_height
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
    /// [BufferLine]s (or paragraphs) of text in the buffer
    pub lines: Vec<BufferLine>,
    metrics: Metrics,
    width: i32,
    height: i32,
    scroll: i32,
    /// True if a redraw is requires. Set to false after processing
    redraw: bool,
    wrap: Wrap,
    ellipsize: Ellipsize,
}

impl<'a> Buffer<'a> {
    /// Create a new [`Buffer`] with the provided [`FontSystem`] and [`Metrics`]
    pub fn new(font_system: &'a FontSystem, metrics: Metrics) -> Self {
        assert_ne!(metrics.line_height, 0, "line height cannot be 0");

        let mut buffer = Self {
            font_system,
            lines: Vec::new(),
            metrics,
            width: 0,
            height: 0,
            scroll: 0,
            redraw: false,
            wrap: Wrap::Word,
            ellipsize: Ellipsize::End(HeightLimit::Default),
        };
        buffer.set_text("", Attrs::new());
        buffer
    }

    fn relayout(&mut self) {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        for line in &mut self.lines {
            if line.shape_opt().is_some() {
                line.reset_layout();
                line.layout(
                    self.font_system,
                    self.metrics.font_size,
                    self.width,
                    self.wrap,
                    self.ellipsize,
                );
            }
        }

        self.redraw = true;

        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        log::debug!("relayout: {:?}", instant.elapsed());
    }

    /// Pre-shape lines in the buffer, up to `lines`, return actual number of layout lines
    pub fn shape_until(&mut self, lines: i32) -> i32 {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

        let mut reshaped = 0;
        let mut total_layout = 0;
        for line in &mut self.lines {
            if total_layout >= lines {
                break;
            }

            if line.shape_opt().is_none() {
                reshaped += 1;
            }
            let layout = line.layout(
                self.font_system,
                self.metrics.font_size,
                self.width,
                self.wrap,
                self.ellipsize,
            );
            total_layout += layout.len() as i32;
        }

        if reshaped > 0 {
            #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
            log::debug!("shape_until {}: {:?}", reshaped, instant.elapsed());
            self.redraw = true;
        }

        total_layout
    }

    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(&mut self, cursor: Cursor) {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        let instant = std::time::Instant::now();

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
                self.width,
                self.wrap,
                self.ellipsize,
            );
            if line_i == cursor.line {
                let layout_cursor = self.layout_cursor(&cursor);
                layout_i += layout_cursor.layout as i32;
                break;
            } else {
                layout_i += layout.len() as i32;
            }
        }

        if reshaped > 0 {
            #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
            log::debug!("shape_until_cursor {}: {:?}", reshaped, instant.elapsed());
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

        self.scroll = cmp::max(0, cmp::min(total_layout - (lines - 1), self.scroll));
    }

    pub fn layout_cursor(&self, cursor: &Cursor) -> LayoutCursor {
        let line = &self.lines[cursor.line];

        //TODO: ensure layout is done?
        let layout = line.layout_opt().as_ref().expect("layout not found");
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

    /// Get [`FontSystem`] used by this [`Buffer`]
    pub fn font_system(&self) -> &'a FontSystem {
        self.font_system
    }

    /// Shape the provided line index and return the result
    pub fn line_shape(&mut self, line_i: usize) -> Option<&ShapeLine> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.shape(self.font_system))
    }

    /// Lay out the provided line index and return the result
    pub fn line_layout(&mut self, line_i: usize) -> Option<&[LayoutLine]> {
        let line = self.lines.get_mut(line_i)?;
        Some(line.layout(
            self.font_system,
            self.metrics.font_size,
            self.width,
            self.wrap,
            self.ellipsize,
        ))
    }

    /// Get the current [`Metrics`]
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// Set the current [`Metrics`]
    pub fn set_metrics(&mut self, metrics: Metrics) {
        if metrics != self.metrics {
            assert_ne!(metrics.font_size, 0, "font size cannot be 0");
            self.metrics = metrics;
            self.relayout();
            self.shape_until_scroll();
        }
    }

    /// Get the current [`Wrap`]
    pub fn wrap(&self) -> Wrap {
        self.wrap
    }

    /// Set the current [`Wrap`]
    pub fn set_wrap(&mut self, wrap: Wrap) {
        if wrap != self.wrap {
            self.wrap = wrap;
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
        let clamped_width = width.max(0);
        let clamped_height = height.max(0);

        if clamped_width != self.width || clamped_height != self.height {
            self.width = clamped_width;
            self.height = clamped_height;
            self.relayout();
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
            self.lines
                .push(BufferLine::new(line.to_string(), AttrsList::new(attrs)));
        }
        // Make sure there is always one line
        if self.lines.is_empty() {
            self.lines
                .push(BufferLine::new(String::new(), AttrsList::new(attrs)));
        }

        self.scroll = 0;

        self.shape_until_scroll();
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
    pub fn layout_runs<'b>(&'b self) -> LayoutRunIter<'a, 'b> {
        LayoutRunIter::new(self)
    }

    /// Convert x, y position to Cursor (hit detection)
    pub fn hit(&self, x: i32, y: i32) -> Option<Cursor> {
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
                        if (run.rtl && x > glyph.x as i32) || (!run.rtl && x < 0) {
                            new_cursor_glyph = 0;
                            new_cursor_char = 0;
                        }
                    }
                    if x >= glyph.x as i32 && x <= (glyph.x + glyph.w) as i32 {
                        new_cursor_glyph = glyph_i;

                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count();
                        let mut egc_x = glyph.x;
                        let egc_w = glyph.w / (total as f32);
                        for (egc_i, egc) in cluster.grapheme_indices(true) {
                            if x >= egc_x as i32 && x <= (egc_x + egc_w) as i32 {
                                new_cursor_char = egc_i;

                                let right_half = x >= (egc_x + egc_w / 2.0) as i32;
                                if right_half != glyph.level.is_rtl() {
                                    // If clicking on last half of glyph, move cursor past glyph
                                    new_cursor_char += egc.len();
                                    new_cursor_affinity = Affinity::Before;
                                }
                                break 'hit;
                            }
                            egc_x += egc_w;
                        }

                        let right_half = x >= (glyph.x + glyph.w / 2.0) as i32;
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

    /// Draw the buffer
    #[cfg(feature = "swash")]
    pub fn draw<F>(&self, cache: &mut crate::SwashCache, color: Color, mut f: F)
    where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        for run in self.layout_runs() {
            for glyph in run.glyphs.iter() {
                let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => color,
                };

                cache.with_pixels(cache_key, glyph_color, |x, y, color| {
                    f(x_int + x, run.line_y + y_int + y, 1, 1, color);
                });
            }
        }
    }
}
