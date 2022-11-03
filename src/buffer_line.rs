use crate::{AttrsList, FontSystem, LayoutLine, ShapeLine};

/// A line (or paragraph) of text that is shaped and laid out
pub struct BufferLine {
    //TODO: make this not pub(crate)
    text: String,
    attrs_list: AttrsList,
    wrap_simple: bool,
    shape_opt: Option<ShapeLine>,
    layout_opt: Option<Vec<LayoutLine>>,
}

impl BufferLine {
    /// Create a new line with the given text and attributes list
    /// Cached shaping and layout can be done using the [Self::shape] and
    /// [Self::layout] functions
    pub fn new<T: Into<String>>(text: T, attrs_list: AttrsList) -> Self {
        Self {
            text: text.into(),
            attrs_list,
            wrap_simple: false,
            shape_opt: None,
            layout_opt: None,
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
    pub fn set_text<T: AsRef<str> + Into<String>>(&mut self, text: T, attrs_list: AttrsList) -> bool {
        if text.as_ref() != &self.text || attrs_list != self.attrs_list {
            self.text = text.into();
            self.attrs_list = attrs_list;
            self.reset();
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
            self.reset();
            true
        } else {
            false
        }
    }

    /// Get simple wrapping setting (wrap by characters only)
    pub fn wrap_simple(&self) -> bool {
        self.wrap_simple
    }

    /// Set simple wrapping setting (wrap by characters only)
    ///
    /// Will reset shape and layout if it differs from current simple wrapping setting.
    /// Returns true if the line was reset
    pub fn set_wrap_simple(&mut self, wrap_simple: bool) -> bool {
        if wrap_simple != self.wrap_simple {
            self.wrap_simple = wrap_simple;
            self.reset();
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
            self.attrs_list.add_span(len..len + other.text().len(), other.attrs_list.defaults());
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

        let mut new = Self::new(text, attrs_list);
        new.wrap_simple = self.wrap_simple;
        new
    }

    /// Reset shaping and layout information
    //TODO: make this private
    pub fn reset(&mut self) {
        self.shape_opt = None;
        self.layout_opt = None;
    }

    /// Reset only layout information
    pub fn reset_layout(&mut self) {
        self.layout_opt = None;
    }

    /// Check if shaping and layout information is cleared
    pub fn is_reset(&self) -> bool {
        self.shape_opt.is_none()
    }

    /// Shape line, will cache results
    pub fn shape(&mut self, font_system: &FontSystem) -> &ShapeLine {
        if self.shape_opt.is_none() {
            self.shape_opt = Some(ShapeLine::new(font_system, &self.text, &self.attrs_list));
            self.layout_opt = None;
        }
        self.shape_opt.as_ref().unwrap()
    }

    /// Get line shaping cache
    pub fn shape_opt(&self) -> &Option<ShapeLine> {
        &self.shape_opt
    }

    /// Layout line, will cache results
    pub fn layout(&mut self, font_system: &FontSystem, font_size: i32, width: i32) -> &[LayoutLine] {
        if self.layout_opt.is_none() {
            let wrap_simple = self.wrap_simple;
            let shape = self.shape(font_system);
            let layout = shape.layout(
                font_size,
                width,
                wrap_simple
            );
            self.layout_opt = Some(layout);
        }
        self.layout_opt.as_ref().unwrap()
    }

    /// Get line layout cache
    pub fn layout_opt(&self) -> &Option<Vec<LayoutLine>> {
        &self.layout_opt
    }
}
