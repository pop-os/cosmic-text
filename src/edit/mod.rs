#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::cmp;

#[cfg(feature = "swash")]
use crate::Color;
use crate::{AttrsList, BorrowedWithFontSystem, Buffer, Cursor, FontSystem};

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Move cursor to previous character ([Self::Left] in LTR, [Self::Right] in RTL)
    Previous,
    /// Move cursor to next character ([Self::Right] in LTR, [Self::Left] in RTL)
    Next,
    /// Move cursor left
    Left,
    /// Move cursor right
    Right,
    /// Move cursor up
    Up,
    /// Move cursor down
    Down,
    /// Move cursor to start of line
    Home,
    /// Move cursor to start of line, skipping whitespace
    SoftHome,
    /// Move cursor to end of line
    End,
    /// Move cursor to start of paragraph
    ParagraphStart,
    /// Move cursor to end of paragraph
    ParagraphEnd,
    /// Move cursor up one page
    PageUp,
    /// Move cursor down one page
    PageDown,
    /// Move cursor up or down by a number of pixels
    Vertical(i32),
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
    /// Mouse drag to specified position
    Drag {
        x: i32,
        y: i32,
    },
    /// Scroll specified number of lines
    Scroll {
        lines: i32,
    },
    /// Move cursor to previous word boundary
    PreviousWord,
    /// Move cursor to next word boundary
    NextWord,
    /// Move cursor to next word boundary to the left
    LeftWord,
    /// Move cursor to next word boundary to the right
    RightWord,
    /// Move cursor to the start of the document
    BufferStart,
    /// Move cursor to the end of the document
    BufferEnd,
    /// Move cursor to specific line
    GotoLine(usize),
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
    //TODO: Select block
}

/// A trait to allow easy replacements of [`Editor`], like `SyntaxEditor`
pub trait Edit {
    /// Mutably borrows `self` together with an [`FontSystem`] for more convenient methods
    fn borrow_with<'a>(
        &'a mut self,
        font_system: &'a mut FontSystem,
    ) -> BorrowedWithFontSystem<'a, Self>
    where
        Self: Sized,
    {
        BorrowedWithFontSystem {
            inner: self,
            font_system,
        }
    }

    /// Get the internal [`Buffer`]
    fn buffer(&self) -> &Buffer;

    /// Get the internal [`Buffer`], mutably
    fn buffer_mut(&mut self) -> &mut Buffer;

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
                let end_index = self.buffer().lines[end_line].text().len();
                Some((Cursor::new(start_line, 0), Cursor::new(end_line, end_index)))
            }
        }
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
    fn shape_as_needed(&mut self, font_system: &mut FontSystem);

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

    /// Perform an [Action] on the editor
    fn action(&mut self, font_system: &mut FontSystem, action: Action);

    /// Draw the editor
    #[cfg(feature = "swash")]
    fn draw<F>(
        &self,
        font_system: &mut FontSystem,
        cache: &mut crate::SwashCache,
        color: Color,
        f: F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color);
}

impl<'a, T: Edit> BorrowedWithFontSystem<'a, T> {
    /// Get the internal [`Buffer`], mutably
    pub fn buffer_mut(&mut self) -> BorrowedWithFontSystem<Buffer> {
        BorrowedWithFontSystem {
            inner: self.inner.buffer_mut(),
            font_system: self.font_system,
        }
    }

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    pub fn shape_as_needed(&mut self) {
        self.inner.shape_as_needed(self.font_system);
    }

    /// Perform an [Action] on the editor
    pub fn action(&mut self, action: Action) {
        self.inner.action(self.font_system, action);
    }

    /// Draw the editor
    #[cfg(feature = "swash")]
    pub fn draw<F>(&mut self, cache: &mut crate::SwashCache, color: Color, f: F)
    where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        self.inner.draw(self.font_system, cache, color, f);
    }
}
