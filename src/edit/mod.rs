use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::cmp;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Attrs, AttrsList, BorrowedWithFontSystem, Buffer, BufferLine, Cursor, FontSystem, InlineBox,
    InlineBoxId, LineEnding, Motion,
};

pub use self::editor::*;
mod editor;

#[cfg(feature = "syntect")]
pub use self::syntect::*;
#[cfg(feature = "syntect")]
mod syntect;

#[cfg(feature = "vi")]
pub use self::vi::*;
#[cfg(feature = "vi")]
mod vi;

/// An action to perform on an [`Editor`]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Move the cursor with some motion
    Motion(Motion),
    /// Escape, clears selection
    Escape,
    /// Insert character at cursor
    Insert(char),
    /// Create new line
    Enter,
    /// Delete text behind cursor
    Backspace,
    /// Delete text in front of cursor
    Delete,
    // Indent text (typically Tab)
    Indent,
    // Unindent text (typically Shift+Tab)
    Unindent,
    /// Mouse click at specified position
    Click {
        x: i32,
        y: i32,
    },
    /// Mouse double click at specified position
    DoubleClick {
        x: i32,
        y: i32,
    },
    /// Mouse triple click at specified position
    TripleClick {
        x: i32,
        y: i32,
    },
    /// Mouse drag to specified position
    Drag {
        x: i32,
        y: i32,
    },
    /// Scroll specified number of pixels
    Scroll {
        pixels: f32,
    },
}

#[derive(Debug)]
pub enum BufferRef<'buffer> {
    Owned(Buffer),
    Borrowed(&'buffer mut Buffer),
    Arc(Arc<Buffer>),
}

impl Clone for BufferRef<'_> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(buffer) => Self::Owned(buffer.clone()),
            Self::Borrowed(buffer) => Self::Owned((*buffer).clone()),
            Self::Arc(buffer) => Self::Arc(buffer.clone()),
        }
    }
}

impl From<Buffer> for BufferRef<'_> {
    fn from(buffer: Buffer) -> Self {
        Self::Owned(buffer)
    }
}

impl<'buffer> From<&'buffer mut Buffer> for BufferRef<'buffer> {
    fn from(buffer: &'buffer mut Buffer) -> Self {
        Self::Borrowed(buffer)
    }
}

impl From<Arc<Buffer>> for BufferRef<'_> {
    fn from(arc: Arc<Buffer>) -> Self {
        Self::Arc(arc)
    }
}

/// Join slices of lines into one string, with each line's ending between
/// them, and an attrs list using the same byte offsets as that string.
pub(crate) fn text_with_attrs<'a>(
    lines: impl IntoIterator<Item = (&'a BufferLine, core::ops::Range<usize>)>,
) -> (String, AttrsList) {
    let mut text = String::new();
    let mut attrs_list: Option<AttrsList> = None;
    let mut last_ending: Option<LineEnding> = None;
    for (line, range) in lines {
        if let Some(ending) = last_ending {
            text.push_str(ending.as_str());
        }
        let attrs_list =
            attrs_list.get_or_insert_with(|| AttrsList::new(&line.attrs_list().defaults()));
        let mut part = line.attrs_list().clone().split_off(range.start);
        part.split_off(range.end - range.start);
        let offset = text.len();
        if !range.is_empty() && part.defaults() != attrs_list.defaults() {
            attrs_list.add_span(offset..offset + range.len(), &part.defaults());
        }
        for (span, attrs) in part.spans_iter() {
            attrs_list.add_span(offset + span.start..offset + span.end, &attrs.as_attrs());
        }
        text.push_str(&line.text()[range]);
        last_ending = Some(line.ending());
    }
    let attrs_list = attrs_list.unwrap_or_else(|| AttrsList::new(&Attrs::new()));
    (text, attrs_list)
}

/// A unique change to an editor
#[derive(Clone, Debug)]
pub struct ChangeItem {
    /// Cursor indicating start of change
    pub start: Cursor,
    /// Cursor indicating end of change
    pub end: Cursor,
    /// Text to be inserted or deleted
    pub text: String,
    /// Attributes of `text`, using the same byte offsets. `None` derives them
    /// from the surrounding text on insert, like [`Edit::insert_string`].
    /// Deletions always record them so that undo restores inline boxes.
    pub attrs_list: Option<AttrsList>,
    /// Insert if true, delete if false
    pub insert: bool,
}

impl ChangeItem {
    // Reverse change item (in place)
    pub fn reverse(&mut self) {
        self.insert = !self.insert;
    }
}

/// A set of change items grouped into one logical change
#[derive(Clone, Debug, Default)]
pub struct Change {
    /// Change items grouped into one change
    pub items: Vec<ChangeItem>,
}

impl Change {
    // Reverse change (in place)
    pub fn reverse(&mut self) {
        self.items.reverse();
        for item in &mut self.items {
            item.reverse();
        }
    }
}

/// Selection mode
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    /// No selection
    None,
    /// Normal selection
    Normal(Cursor),
    /// Select by lines
    Line(Cursor),
    /// Select by words
    Word(Cursor),
    //TODO: Select block
}

/// A trait to allow easy replacements of [`Editor`], like `SyntaxEditor`
pub trait Edit<'buffer> {
    /// Mutably borrows `self` together with an [`FontSystem`] for more convenient methods
    fn borrow_with<'font_system>(
        &'font_system mut self,
        font_system: &'font_system mut FontSystem,
    ) -> BorrowedWithFontSystem<'font_system, Self>
    where
        Self: Sized,
    {
        BorrowedWithFontSystem {
            inner: self,
            font_system,
        }
    }

    /// Get the internal [`BufferRef`]
    fn buffer_ref(&self) -> &BufferRef<'buffer>;

    /// Get the internal [`BufferRef`]
    fn buffer_ref_mut(&mut self) -> &mut BufferRef<'buffer>;

    /// Get the internal [`Buffer`]
    fn with_buffer<F: FnOnce(&Buffer) -> T, T>(&self, f: F) -> T {
        match self.buffer_ref() {
            BufferRef::Owned(buffer) => f(buffer),
            BufferRef::Borrowed(buffer) => f(buffer),
            BufferRef::Arc(buffer) => f(buffer),
        }
    }

    /// Get the internal [`Buffer`], mutably
    fn with_buffer_mut<F: FnOnce(&mut Buffer) -> T, T>(&mut self, f: F) -> T {
        match self.buffer_ref_mut() {
            BufferRef::Owned(buffer) => f(buffer),
            BufferRef::Borrowed(buffer) => f(buffer),
            BufferRef::Arc(buffer) => f(Arc::make_mut(buffer)),
        }
    }

    /// Get the [`Buffer`] redraw flag
    fn redraw(&self) -> bool {
        self.with_buffer(super::buffer::Buffer::redraw)
    }

    /// Set the [`Buffer`] redraw flag
    fn set_redraw(&mut self, redraw: bool) {
        self.with_buffer_mut(|buffer| buffer.set_redraw(redraw));
    }

    /// Get the current cursor
    fn cursor(&self) -> Cursor;

    /// Set the current cursor
    fn set_cursor(&mut self, cursor: Cursor);

    /// Get the current selection position
    fn selection(&self) -> Selection;

    /// Set the current selection position
    fn set_selection(&mut self, selection: Selection);

    /// Get the bounds of the current selection
    //TODO: will not work with Block select
    fn selection_bounds(&self) -> Option<(Cursor, Cursor)> {
        self.with_buffer(|buffer| {
            let cursor = self.cursor();
            match self.selection() {
                Selection::None => None,
                Selection::Normal(select) => match select.line.cmp(&cursor.line) {
                    cmp::Ordering::Greater => Some((cursor, select)),
                    cmp::Ordering::Less => Some((select, cursor)),
                    cmp::Ordering::Equal => {
                        /* select.line == cursor.line */
                        if select.index < cursor.index {
                            Some((select, cursor))
                        } else {
                            /* select.index >= cursor.index */
                            Some((cursor, select))
                        }
                    }
                },
                Selection::Line(select) => {
                    let start_line = cmp::min(select.line, cursor.line);
                    let end_line = cmp::max(select.line, cursor.line);
                    let end_index = buffer.lines[end_line].text().len();
                    Some((Cursor::new(start_line, 0), Cursor::new(end_line, end_index)))
                }
                Selection::Word(select) => {
                    let (mut start, mut end) = match select.line.cmp(&cursor.line) {
                        cmp::Ordering::Greater => (cursor, select),
                        cmp::Ordering::Less => (select, cursor),
                        cmp::Ordering::Equal => {
                            /* select.line == cursor.line */
                            if select.index < cursor.index {
                                (select, cursor)
                            } else {
                                /* select.index >= cursor.index */
                                (cursor, select)
                            }
                        }
                    };

                    // Move start to beginning of word
                    {
                        let line = &buffer.lines[start.line];
                        start.index = line
                            .text()
                            .unicode_word_indices()
                            .rev()
                            .map(|(i, _)| i)
                            .find(|&i| i < start.index)
                            .unwrap_or(0);
                    }

                    // Move end to end of word
                    {
                        let line = &buffer.lines[end.line];
                        end.index = line
                            .text()
                            .unicode_word_indices()
                            .map(|(i, word)| i + word.len())
                            .find(|&i| i > end.index)
                            .unwrap_or_else(|| line.text().len());
                    }

                    Some((start, end))
                }
            }
        })
    }

    /// Get the current automatic indentation setting
    fn auto_indent(&self) -> bool;

    /// Enable or disable automatic indentation
    fn set_auto_indent(&mut self, auto_indent: bool);

    /// Get the current tab width
    fn tab_width(&self) -> u16;

    /// Set the current tab width. A `tab_width` of 0 is not allowed, and will be ignored
    fn set_tab_width(&mut self, tab_width: u16);

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    fn shape_as_needed(&mut self, font_system: &mut FontSystem, prune: bool);

    /// Delete text starting at start Cursor and ending at end Cursor
    fn delete_range(&mut self, start: Cursor, end: Cursor);

    /// Insert text at specified cursor with specified `attrs_list`
    fn insert_at(&mut self, cursor: Cursor, data: &str, attrs_list: Option<AttrsList>) -> Cursor;

    /// Copy the selection as plain text.
    ///
    /// Inline box placeholders are omitted, so the result is safe to hand to
    /// the system clipboard. Use [`Edit::copy_selection_with_attrs`] to keep
    /// boxes and styling for a paste back into an editor.
    fn copy_selection(&self) -> Option<String>;

    /// Copy the selection as text plus the attributes of that text, with
    /// inline box placeholders and their boxes kept.
    ///
    /// The attrs list uses the same byte offsets as the text, so the pair can
    /// go straight back into [`Edit::insert_string`]. Before pasting, pass the
    /// attrs list through [`Buffer::remint_inline_box_ids`] so the pasted
    /// boxes get ids of their own:
    ///
    /// ```ignore
    /// let (text, mut attrs) = editor.copy_selection_with_attrs()?;
    /// let renamed = editor.with_buffer_mut(|b| b.remint_inline_box_ids(&mut attrs));
    /// editor.insert_string(&text, Some(attrs));
    /// ```
    fn copy_selection_with_attrs(&self) -> Option<(String, AttrsList)> {
        let (start, end) = self.selection_bounds()?;
        Some(self.with_buffer(|buffer| {
            let lines = buffer.lines[start.line..=end.line]
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let line_i = start.line + i;
                    let from = if line_i == start.line { start.index } else { 0 };
                    let to = if line_i == end.line {
                        end.index
                    } else {
                        line.text().len()
                    };
                    (line, from..to)
                });
            text_with_attrs(lines)
        }))
    }

    /// Delete selection, adjusting cursor and returning true if there was a selection
    // Also used by backspace, delete, insert, and enter when there is a selection
    fn delete_selection(&mut self) -> bool;

    /// Insert a string at the current cursor or replacing the current selection with the given
    /// attributes, or with the previous character's attributes if None is given.
    fn insert_string(&mut self, data: &str, attrs_list: Option<AttrsList>) {
        self.delete_selection();
        let new_cursor = self.insert_at(self.cursor(), data, attrs_list);
        self.set_cursor(new_cursor);
    }

    /// Insert an [`InlineBox`] placeholder at the current cursor, replacing any
    /// active selection.
    fn insert_inline_box(&mut self, inline_box: InlineBox) {
        self.delete_selection();
        let cursor = self.cursor();
        let attrs_list = self.with_buffer(|buffer| {
            let base = buffer
                .lines
                .get(cursor.line)
                .map(|line| line.attrs_list().get_span(cursor.index.saturating_sub(1)))
                .unwrap_or_else(Attrs::new);
            AttrsList::new(&base.inline_box(inline_box))
        });
        let new_cursor = self.insert_at(cursor, "\u{FFFC}", Some(attrs_list));
        self.set_cursor(new_cursor);
    }

    /// Remove the inline box with the given `id`, if present.
    fn remove_inline_box(&mut self, id: InlineBoxId) -> bool {
        let range = self.with_buffer(|buffer| {
            let (start, _) = buffer.inline_boxes().find(|(_, ib)| ib.id == id)?;
            let end = Cursor::new(start.line, start.index + "\u{FFFC}".len());
            Some((start, end))
        });

        match range {
            Some((start, end)) => {
                self.delete_range(start, end);
                self.set_cursor(start);
                true
            }
            None => false,
        }
    }

    /// Apply a change
    fn apply_change(&mut self, change: &Change) -> bool;

    /// Start collecting change
    fn start_change(&mut self);

    /// Get completed change
    fn finish_change(&mut self) -> Option<Change>;

    /// Perform an [Action] on the editor
    fn action(&mut self, font_system: &mut FontSystem, action: Action);

    /// Get X and Y position of the top left corner of the cursor
    fn cursor_position(&self) -> Option<(i32, i32)>;
}

impl<'buffer, E: Edit<'buffer>> BorrowedWithFontSystem<'_, E> {
    /// Get the internal [`Buffer`], mutably
    pub fn with_buffer_mut<F: FnOnce(&mut BorrowedWithFontSystem<Buffer>) -> T, T>(
        &mut self,
        f: F,
    ) -> T {
        self.inner.with_buffer_mut(|buffer| {
            let mut borrowed = BorrowedWithFontSystem {
                inner: buffer,
                font_system: self.font_system,
            };
            f(&mut borrowed)
        })
    }

    /// Set the current tab width. A `tab_width` of 0 is not allowed, and will be ignored
    pub fn set_tab_width(&mut self, tab_width: u16) {
        self.inner.set_tab_width(tab_width);
    }

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    pub fn shape_as_needed(&mut self, prune: bool) {
        self.inner.shape_as_needed(self.font_system, prune);
    }

    /// Perform an [Action] on the editor
    pub fn action(&mut self, action: Action) {
        self.inner.action(self.font_system, action);
    }

    /// Insert an [`InlineBox`] at the cursor
    pub fn insert_inline_box(&mut self, inline_box: InlineBox) {
        self.inner.insert_inline_box(inline_box);
    }

    /// Remove the inline box with the given `id`
    pub fn remove_inline_box(&mut self, id: InlineBoxId) -> bool {
        self.inner.remove_inline_box(id)
    }
}
