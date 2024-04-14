use alloc::sync::Arc;
use core::ops::Range;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::cmp;
use unicode_segmentation::UnicodeSegmentation;

use crate::{AttrsList, AttrsOwned, BorrowedWithFontSystem, Buffer, Cursor, FontSystem, Motion};

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Move the cursor with some motion
    Motion {
        motion: Motion,
        select: bool,
    },
    /// Escape, clears selection
    Escape,
    /// Select text from start to end
    SelectAll,
    /// Insert character at cursor
    Insert(char),
    /// Create new line
    Enter,
    /// Delete text behind cursor
    Backspace,
    /// Delete text behind cursor to next word boundary
    DeleteStartOfWord,
    /// Delete text in front of cursor
    Delete,
    /// Delete text in front of cursor to next word boundary
    DeleteEndOfWord,
    // Indent text (typically Tab)
    Indent,
    // Unindent text (typically Shift+Tab)
    Unindent,
    /// Mouse click at specified position
    Click {
        x: i32,
        y: i32,
        select: bool,
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
    /// Scroll specified number of lines
    Scroll {
        lines: i32,
    },
    /// Set preedit text, replacing any previous preedit text
    ///
    /// If `cursor` is specified, it contains a start and end cursor byte positions
    /// within the preedit. If no cursor is specified for a non-empty preedit,
    /// the cursor should be hidden.
    ///
    /// If `attrs` is specified, these attributes will be assigned to the preedit's span.
    /// However, regardless of `attrs` setting, the preedit's span will always have
    /// `is_preedit` set to `true`.
    SetPreedit {
        preedit: String,
        cursor: Option<(usize, usize)>,
        attrs: Option<AttrsOwned>,
    },
}

#[derive(Debug)]
pub enum BufferRef<'buffer> {
    Owned(Buffer),
    Borrowed(&'buffer mut Buffer),
    Arc(Arc<Buffer>),
}

impl<'buffer> From<Buffer> for BufferRef<'buffer> {
    fn from(buffer: Buffer) -> Self {
        Self::Owned(buffer)
    }
}

impl<'buffer> From<&'buffer mut Buffer> for BufferRef<'buffer> {
    fn from(buffer: &'buffer mut Buffer) -> Self {
        Self::Borrowed(buffer)
    }
}

impl<'buffer> From<Arc<Buffer>> for BufferRef<'buffer> {
    fn from(arc: Arc<Buffer>) -> Self {
        Self::Arc(arc)
    }
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
        for item in self.items.iter_mut() {
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
        self.with_buffer(|buffer| buffer.redraw())
    }

    /// Set the [`Buffer`] redraw flag
    fn set_redraw(&mut self, redraw: bool) {
        self.with_buffer_mut(|buffer| buffer.set_redraw(redraw))
    }

    /// Get the current cursor
    fn cursor(&self) -> Cursor;

    /// Hide or show the cursor
    ///
    /// This should be used to hide the cursor, for example,
    /// when the editor is unfocused, when the text is not editable,
    /// or to implement cursor blinking.
    ///
    /// Note that even after `set_cursor_hidden(false)`, the editor may
    /// choose to hide the cursor based on internal state, for example,
    /// when there is a selection or when there is a preedit without a cursor.
    fn set_cursor_hidden(&mut self, hidden: bool);

    /// Set the current cursor
    fn set_cursor(&mut self, cursor: Cursor);

    /// Returns true if some text is selected
    fn has_selection(&self) -> bool {
        match self.selection() {
            Selection::None => false,
            Selection::Normal(selection) => {
                let cursor = self.cursor();
                selection.line != cursor.line || selection.index != cursor.index
            }
            Selection::Line(selection) => selection.line != self.cursor().line,
            Selection::Word(_) => true,
        }
    }

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
                            .unwrap_or(line.text().len());
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

    /// Insert text at specified cursor with specified attrs_list
    fn insert_at(&mut self, cursor: Cursor, data: &str, attrs_list: Option<AttrsList>) -> Cursor;

    /// Copy selection
    fn copy_selection(&self) -> Option<String>;

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

    /// Apply a change
    fn apply_change(&mut self, change: &Change) -> bool;

    /// Start collecting change
    fn start_change(&mut self);

    /// Get completed change
    fn finish_change(&mut self) -> Option<Change>;

    /// Returns the range of byte indices of the text corresponding
    /// to the preedit
    fn preedit_range(&self) -> Option<Range<usize>>;

    /// Get current preedit text
    fn preedit_text(&self) -> Option<String>;

    /// Perform an [Action] on the editor
    fn action(&mut self, font_system: &mut FontSystem, action: Action);

    /// Get X and Y position of the top left corner of the cursor
    fn cursor_position(&self) -> Option<(i32, i32)>;
}

impl<'font_system, 'buffer, E: Edit<'buffer>> BorrowedWithFontSystem<'font_system, E> {
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

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    pub fn shape_as_needed(&mut self, prune: bool) {
        self.inner.shape_as_needed(self.font_system, prune);
    }

    /// Perform an [Action] on the editor
    pub fn action(&mut self, action: Action) {
        self.inner.action(self.font_system, action);
    }
}
