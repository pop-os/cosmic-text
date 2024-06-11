#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::{
    Align, AttrsList, FontSystem, LayoutLine, LineEnding, ShapeBuffer, ShapeLine, Shaping, Wrap,
};

/// A line (or paragraph) of text that is shaped and laid out
#[derive(Clone, Debug)]
pub struct BufferLine {
    text: String,
    ending: LineEnding,
    attrs_list: AttrsList,
    align: Option<Align>,
    shape_opt: Option<ShapeLine>,
    layout_opt: Option<Vec<LayoutLine>>,
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
            shape_opt: None,
            layout_opt: None,
            shaping,
            metadata: None,
        }
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
    pub fn ending(&self) -> LineEnding {
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
    pub fn attrs_list(&self) -> &AttrsList {
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
    pub fn align(&self) -> Option<Align> {
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
    pub fn append(&mut self, other: Self) {
        let len = self.text.len();
        self.text.push_str(other.text());

        if other.attrs_list.defaults() != self.attrs_list.defaults() {
            // If default formatting does not match, make a new span for it
            self.attrs_list
                .add_span(len..len + other.text().len(), other.attrs_list.defaults());
        }

        for (other_range, attrs) in other.attrs_list.spans() {
            // Add previous attrs spans
            let range = other_range.start + len..other_range.end + len;
            self.attrs_list.add_span(range, attrs.as_attrs());
        }

        self.reset();
    }

    /// Split off new line at index
    pub fn split_off(&mut self, index: usize) -> Self {
        let text = self.text.split_off(index);
        let attrs_list = self.attrs_list.split_off(index);
        self.reset();

        let mut new = Self::new(text, self.ending, attrs_list, self.shaping);
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
        self.shape_opt = None;
        self.reset_layout();
    }

    /// Reset only layout cache
    pub fn reset_layout(&mut self) {
        self.layout_opt = None;
    }

    /// Shape line, will cache results
    pub fn shape(&mut self, font_system: &mut FontSystem, tab_width: u16) -> &ShapeLine {
        self.shape_in_buffer(&mut ShapeBuffer::default(), font_system, tab_width)
    }

    /// Shape a line using a pre-existing shape buffer, will cache results
    pub fn shape_in_buffer(
        &mut self,
        scratch: &mut ShapeBuffer,
        font_system: &mut FontSystem,
        tab_width: u16,
    ) -> &ShapeLine {
        if self.shape_opt.is_none() {
            self.shape_opt = Some(ShapeLine::new_in_buffer(
                scratch,
                font_system,
                &self.text,
                &self.attrs_list,
                self.shaping,
                tab_width,
            ));
            self.layout_opt = None;
        }
        self.shape_opt.as_ref().expect("shape not found")
    }

    /// Get line shaping cache
    pub fn shape_opt(&self) -> &Option<ShapeLine> {
        &self.shape_opt
    }

    /// Layout line, will cache results
    pub fn layout(
        &mut self,
        font_system: &mut FontSystem,
        font_size: f32,
        width_opt: Option<f32>,
        wrap: Wrap,
        match_mono_width: Option<f32>,
        tab_width: u16,
    ) -> &[LayoutLine] {
        self.layout_in_buffer(
            &mut ShapeBuffer::default(),
            font_system,
            font_size,
            width_opt,
            wrap,
            match_mono_width,
            tab_width,
        )
    }

    /// Layout a line using a pre-existing shape buffer, will cache results
    pub fn layout_in_buffer(
        &mut self,
        scratch: &mut ShapeBuffer,
        font_system: &mut FontSystem,
        font_size: f32,
        width_opt: Option<f32>,
        wrap: Wrap,
        match_mono_width: Option<f32>,
        tab_width: u16,
    ) -> &[LayoutLine] {
        if self.layout_opt.is_none() {
            let align = self.align;
            let shape = self.shape_in_buffer(scratch, font_system, tab_width);
            let mut layout = Vec::with_capacity(1);
            shape.layout_to_buffer(
                scratch,
                font_size,
                width_opt,
                wrap,
                align,
                &mut layout,
                match_mono_width,
            );
            self.layout_opt = Some(layout);
        }
        self.layout_opt.as_ref().expect("layout not found")
    }

    /// Get line layout cache
    pub fn layout_opt(&self) -> &Option<Vec<LayoutLine>> {
        &self.layout_opt
    }

    /// Get line metadata. This will be None if [`BufferLine::set_metadata`] has not been called
    /// after the last reset of shaping and layout caches
    pub fn metadata(&self) -> Option<usize> {
        self.metadata
    }

    /// Set line metadata. This is stored until the next line reset
    pub fn set_metadata(&mut self, metadata: usize) {
        self.metadata = Some(metadata);
    }
}
