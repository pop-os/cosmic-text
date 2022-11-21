#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::{Buffer, Cursor};
#[cfg(feature = "swash")]
use crate::Color;

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
    /// Scroll up one page
    PageUp,
    /// Scroll down one page
    PageDown,
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
}

/// A trait to allow easy replacements of [`Editor`], like `SyntaxEditor`
pub trait Edit<'a> {
    /// Get the internal [`Buffer`]
    fn buffer(&self) -> &Buffer<'a>;

    /// Get the internal [`Buffer`], mutably
    fn buffer_mut(&mut self) -> &mut Buffer<'a>;

    /// Get the current cursor position
    fn cursor(&self) -> Cursor;

    /// Get the current selection position
    fn select_opt(&self) -> Option<Cursor>;

    /// Set the current selection position
    fn set_select_opt(&mut self, select_opt: Option<Cursor>);

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    fn shape_as_needed(&mut self);

    /// Copy selection
    fn copy_selection(&mut self) -> Option<String>;

    /// Delete selection, adjusting cursor and returning true if there was a selection
    // Also used by backspace, delete, insert, and enter when there is a selection
    fn delete_selection(&mut self) -> bool;

    /// Perform an [Action] on the editor
    fn action(&mut self, action: Action);

    /// Draw the editor
    #[cfg(feature = "swash")]
    fn draw<F>(&self, cache: &mut crate::SwashCache, color: Color, f: F)
        where F: FnMut(i32, i32, u32, u32, Color);
}
