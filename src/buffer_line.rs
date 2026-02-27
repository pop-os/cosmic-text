#![allow(clippy::too_many_arguments)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::mem;

use crate::{
    Affinity, Align, Attrs, AttrsList, Cached, Cursor, DecorationSpan, Ellipsize, FontSystem,
    Hinting, LayoutGlyph, LayoutLine, LineEnding, ShapeLine, Shaping, Wrap,
};

/// A line (or paragraph) of text that is shaped and laid out
#[derive(Clone, Debug)]
pub struct BufferLine {
    text: String,
    ending: LineEnding,
    attrs_list: AttrsList,
    align: Option<Align>,
    shape_opt: Cached<ShapeLine>,
    layout_opt: Cached<Vec<LayoutLine>>,
    shaping: Shaping,
    metadata: Option<usize>,
}

impl BufferLine {
    /// Create a new line with the given text and attributes list
    /// Cached shaping and layout can be done using the [`Self::shape`] and
    /// [`Self::layout`] functions
    pub fn new<T: Into<String>>(
        text: T,
        ending: LineEnding,
        attrs_list: AttrsList,
        shaping: Shaping,
    ) -> Self {
        Self {
            text: text.into(),
            ending,
            attrs_list,
            align: None,
            shape_opt: Cached::Empty,
            layout_opt: Cached::Empty,
            shaping,
            metadata: None,
        }
    }

    /// Resets the current line with new internal values.
    ///
    /// Avoids deallocating internal caches so they can be reused.
    pub fn reset_new<T: Into<String>>(
        &mut self,
        text: T,
        ending: LineEnding,
        attrs_list: AttrsList,
        shaping: Shaping,
    ) {
        self.text = text.into();
        self.ending = ending;
        self.attrs_list = attrs_list;
        self.align = None;
        self.shape_opt.set_unused();
        self.layout_opt.set_unused();
        self.shaping = shaping;
        self.metadata = None;
    }

    /// Get current text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set text and attributes list
    ///
    /// Will reset shape and layout if it differs from current text and attributes list.
    /// Returns true if the line was reset
    pub fn set_text<T: AsRef<str>>(
        &mut self,
        text: T,
        ending: LineEnding,
        attrs_list: AttrsList,
    ) -> bool {
        let text = text.as_ref();
        if text != self.text || ending != self.ending || attrs_list != self.attrs_list {
            self.text.clear();
            self.text.push_str(text);
            self.ending = ending;
            self.attrs_list = attrs_list;
            self.reset();
            true
        } else {
            false
        }
    }

    /// Consume this line, returning only its text contents as a String.
    pub fn into_text(self) -> String {
        self.text
    }

    /// Get line ending
    pub const fn ending(&self) -> LineEnding {
        self.ending
    }

    /// Set line ending
    ///
    /// Will reset shape and layout if it differs from current line ending.
    /// Returns true if the line was reset
    pub fn set_ending(&mut self, ending: LineEnding) -> bool {
        if ending != self.ending {
            self.ending = ending;
            self.reset_shaping();
            true
        } else {
            false
        }
    }

    /// Get attributes list
    pub const fn attrs_list(&self) -> &AttrsList {
        &self.attrs_list
    }

    /// Set attributes list
    ///
    /// Will reset shape and layout if it differs from current attributes list.
    /// Returns true if the line was reset
    pub fn set_attrs_list(&mut self, attrs_list: AttrsList) -> bool {
        if attrs_list != self.attrs_list {
            self.attrs_list = attrs_list;
            self.reset_shaping();
            true
        } else {
            false
        }
    }

    /// Get the Text alignment
    pub const fn align(&self) -> Option<Align> {
        self.align
    }

    /// Set the text alignment
    ///
    /// Will reset layout if it differs from current alignment.
    /// Setting to None will use `Align::Right` for RTL lines, and `Align::Left` for LTR lines.
    /// Returns true if the line was reset
    pub fn set_align(&mut self, align: Option<Align>) -> bool {
        if align != self.align {
            self.align = align;
            self.reset_layout();
            true
        } else {
            false
        }
    }

    /// Append line at end of this line
    ///
    /// The wrap setting of the appended line will be lost
    pub fn append(&mut self, other: &Self) {
        let len = self.text.len();
        self.text.push_str(other.text());

        // To preserve line endings, we use the one from the other line
        self.ending = other.ending();

        if other.attrs_list.defaults() != self.attrs_list.defaults() {
            // If default formatting does not match, make a new span for it
            self.attrs_list
                .add_span(len..len + other.text().len(), &other.attrs_list.defaults());
        }

        for (other_range, attrs) in other.attrs_list.spans_iter() {
            // Add previous attrs spans
            let range = other_range.start + len..other_range.end + len;
            self.attrs_list.add_span(range, &attrs.as_attrs());
        }

        self.reset();
    }

    /// Split off new line at index
    pub fn split_off(&mut self, index: usize) -> Self {
        let text = self.text.split_off(index);
        let attrs_list = self.attrs_list.split_off(index);
        self.reset();

        let mut new = Self::new(text, self.ending, attrs_list, self.shaping);
        // To preserve line endings, it moves to the new line
        self.ending = LineEnding::None;
        new.align = self.align;
        new
    }

    /// Reset shaping, layout, and metadata caches
    pub fn reset(&mut self) {
        self.metadata = None;
        self.reset_shaping();
    }

    /// Reset shaping and layout caches
    pub fn reset_shaping(&mut self) {
        self.shape_opt.set_unused();
        self.reset_layout();
    }

    /// Reset only layout cache
    pub fn reset_layout(&mut self) {
        self.layout_opt.set_unused();
    }

    /// Shape line, will cache results
    #[allow(clippy::missing_panics_doc)]
    pub fn shape(&mut self, font_system: &mut FontSystem, tab_width: u16) -> &ShapeLine {
        if self.shape_opt.is_unused() {
            let mut line = self
                .shape_opt
                .take_unused()
                .unwrap_or_else(ShapeLine::empty);
            line.build(
                font_system,
                &self.text,
                &self.attrs_list,
                self.shaping,
                tab_width,
            );
            self.shape_opt.set_used(line);
            self.layout_opt.set_unused();
        }
        self.shape_opt.get().expect("shape not found")
    }

    /// Get line shaping cache
    pub const fn shape_opt(&self) -> Option<&ShapeLine> {
        self.shape_opt.get()
    }

    /// Layout line, will cache results
    #[allow(clippy::missing_panics_doc)]
    pub fn layout(
        &mut self,
        font_system: &mut FontSystem,
        font_size: f32,
        width_opt: Option<f32>,
        wrap: Wrap,
        ellipsize: Ellipsize,
        match_mono_width: Option<f32>,
        tab_width: u16,
        hinting: Hinting,
    ) -> &[LayoutLine] {
        if self.layout_opt.is_unused() {
            let align = self.align;
            let mut layout = self
                .layout_opt
                .take_unused()
                .unwrap_or_else(|| Vec::with_capacity(1));
            let shape = self.shape(font_system, tab_width);
            shape.layout_to_buffer(
                &mut font_system.shape_buffer,
                font_size,
                width_opt,
                wrap,
                ellipsize,
                align,
                &mut layout,
                match_mono_width,
                hinting,
            );
            self.layout_opt.set_used(layout);
        }
        self.layout_opt.get().expect("layout not found")
    }

    /// Get line layout cache
    pub const fn layout_opt(&self) -> Option<&Vec<LayoutLine>> {
        self.layout_opt.get()
    }

    /// Get the visible layout runs for rendering and other tasks
    pub fn layout_runs(&self, height_opt: Option<f32>, line_height: f32) -> LayoutRunIter<'_> {
        LayoutRunIter::new(core::slice::from_ref(self), height_opt, line_height, 0.0, 0)
    }

    /// Get line metadata. This will be None if [`BufferLine::set_metadata`] has not been called
    /// after the last reset of shaping and layout caches
    pub const fn metadata(&self) -> Option<usize> {
        self.metadata
    }

    /// Set line metadata. This is stored until the next line reset
    pub fn set_metadata(&mut self, metadata: usize) {
        self.metadata = Some(metadata);
    }

    /// Makes an empty buffer line.
    ///
    /// The buffer line is in an invalid state after this is called. See [`Self::reset_new`].
    pub(crate) fn empty() -> Self {
        Self {
            text: String::default(),
            ending: LineEnding::None,
            attrs_list: AttrsList::new(&Attrs::new()),
            align: None,
            shape_opt: Cached::Empty,
            layout_opt: Cached::Empty,
            shaping: Shaping::Advanced,
            metadata: None,
        }
    }

    /// Reclaim attributes list memory that isn't needed any longer.
    ///
    /// The buffer line is in an invalid state after this is called. See [`Self::reset_new`].
    pub(crate) fn reclaim_attrs(&mut self) -> AttrsList {
        mem::replace(&mut self.attrs_list, AttrsList::new(&Attrs::new()))
    }

    /// Reclaim text memory that isn't needed any longer.
    ///
    /// The buffer line is in an invalid state after this is called. See [`Self::reset_new`].
    pub(crate) fn reclaim_text(&mut self) -> String {
        let mut text = mem::take(&mut self.text);
        text.clear();
        text
    }
}

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
    /// Text decoration spans covering ranges of glyphs
    pub decorations: &'a [DecorationSpan],
    /// Y offset to baseline of line
    pub line_y: f32,
    /// Y offset to top of line
    pub line_top: f32,
    /// Y offset to next line
    pub line_height: f32,
    /// Width of line
    pub line_w: f32,
}

impl LayoutRun<'_> {
    /// Return the pixel span `Some((x_left, x_width))` of the highlighted area between `cursor_start`
    /// and `cursor_end` within this run, or None if the cursor range does not intersect this run.
    /// This may return widths of zero if `cursor_start == cursor_end`, if the run is empty, or if the
    /// region's left start boundary is the same as the cursor's end boundary or vice versa.
    #[allow(clippy::missing_panics_doc)]
    pub fn highlight(&self, cursor_start: Cursor, cursor_end: Cursor) -> Option<(f32, f32)> {
        let mut x_start = None;
        let mut x_end = None;
        let rtl_factor = if self.rtl { 1. } else { 0. };
        let ltr_factor = 1. - rtl_factor;
        for glyph in self.glyphs {
            let cursor = self.cursor_from_glyph_left(glyph);
            if cursor >= cursor_start && cursor <= cursor_end {
                if x_start.is_none() {
                    x_start = Some(glyph.x + glyph.w.mul_add(rtl_factor, 0.0));
                }
                x_end = Some(glyph.x + glyph.w.mul_add(rtl_factor, 0.0));
            }
            let cursor = self.cursor_from_glyph_right(glyph);
            if cursor >= cursor_start && cursor <= cursor_end {
                if x_start.is_none() {
                    x_start = Some(glyph.x + glyph.w.mul_add(ltr_factor, 0.0));
                }
                x_end = Some(glyph.x + glyph.w.mul_add(ltr_factor, 0.0));
            }
        }
        x_start.map(|x_start| {
            let x_end = x_end.expect("end of cursor not found");
            let (x_start, x_end) = if x_start < x_end {
                (x_start, x_end)
            } else {
                (x_end, x_start)
            };
            (x_start, x_end - x_start)
        })
    }

    pub(crate) const fn cursor_from_glyph_left(&self, glyph: &LayoutGlyph) -> Cursor {
        if self.rtl {
            Cursor::new_with_affinity(self.line_i, glyph.end, Affinity::Before)
        } else {
            Cursor::new_with_affinity(self.line_i, glyph.start, Affinity::After)
        }
    }

    pub(crate) const fn cursor_from_glyph_right(&self, glyph: &LayoutGlyph) -> Cursor {
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
    lines: &'b [BufferLine],
    height_opt: Option<f32>,
    line_height: f32,
    scroll: f32,
    line_i: usize,
    layout_i: usize,
    total_height: f32,
    line_top: f32,
}

impl<'b> LayoutRunIter<'b> {
    pub const fn new(
        lines: &'b [BufferLine],
        height_opt: Option<f32>,
        line_height: f32,
        scroll: f32,
        start: usize,
    ) -> Self {
        Self {
            lines,
            height_opt,
            line_height,
            scroll,
            line_i: start,
            layout_i: 0,
            total_height: 0.0,
            line_top: 0.0,
        }
    }
}

impl<'b> Iterator for LayoutRunIter<'b> {
    type Item = LayoutRun<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.lines.get(self.line_i) {
            let shape = line.shape_opt()?;
            let layout = line.layout_opt()?;
            while let Some(layout_line) = layout.get(self.layout_i) {
                self.layout_i += 1;

                let line_height = layout_line.line_height_opt.unwrap_or(self.line_height);
                self.total_height += line_height;

                let line_top = self.line_top - self.scroll;
                let glyph_height = layout_line.max_ascent + layout_line.max_descent;
                let centering_offset = (line_height - glyph_height) / 2.0;
                let line_y = line_top + centering_offset + layout_line.max_ascent;
                if let Some(height) = self.height_opt {
                    if line_y - layout_line.max_ascent > height {
                        return None;
                    }
                }
                self.line_top += line_height;
                if line_y + layout_line.max_descent < 0.0 {
                    continue;
                }

                return Some(LayoutRun {
                    line_i: self.line_i,
                    text: line.text(),
                    rtl: shape.rtl,
                    glyphs: &layout_line.glyphs,
                    decorations: &layout_line.decorations,
                    line_y,
                    line_top,
                    line_height,
                    line_w: layout_line.w,
                });
            }
            self.line_i += 1;
            self.layout_i = 0;
        }

        None
    }
}
