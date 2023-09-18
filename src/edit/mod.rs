#[cfg(not(feature = "std"))]
use alloc::string::String;

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
    /// Mouse click at specified position
    Click { x: i32, y: i32 },
    /// Mouse drag to specified position
    Drag { x: i32, y: i32 },
    /// Scroll specified number of lines
    Scroll { lines: i32 },
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
    fn select_opt(&self) -> Option<Cursor>;

    /// Set the current selection position
    fn set_select_opt(&mut self, select_opt: Option<Cursor>);

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    fn shape_as_needed(&mut self, font_system: &mut FontSystem);

    /// Copy selection
    fn copy_selection(&self) -> Option<String>;

    /// Delete selection, adjusting cursor and returning true if there was a selection
    // Also used by backspace, delete, insert, and enter when there is a selection
    fn delete_selection(&mut self) -> bool;

    /// Insert a string at the current cursor or replacing the current selection with the given
    /// attributes, or with the previous character's attributes if None is given.
    fn insert_string(&mut self, data: &str, attrs_list: Option<AttrsList>);

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
