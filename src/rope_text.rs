// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rope-based text storage for efficient handling of large files.
//!
//! This module provides a wrapper around the `ropey` crate's `Rope` type,
//! optimized for line-based operations needed by cosmic-text.

#[cfg(not(feature = "std"))]
use alloc::{borrow::Cow, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::borrow::Cow;

use ropey::{Rope, RopeSlice};

use crate::LineEnding;

/// A rope-based text storage optimized for line operations.
///
/// This provides efficient O(log n) access to lines and supports
/// efficient insertion and deletion at arbitrary positions.
#[derive(Debug, Clone)]
pub struct RopeText {
    rope: Rope,
    /// Cached line endings for each line (None means we need to detect)
    line_endings: Vec<Option<LineEnding>>,
}

impl Default for RopeText {
    fn default() -> Self {
        Self::new()
    }
}

impl RopeText {
    /// Create a new empty `RopeText`.
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            line_endings: Vec::new(),
        }
    }

    /// Create a `RopeText` from a string.
    pub fn from_str(text: &str) -> Self {
        let rope = Rope::from_str(text);
        let line_count = rope.len_lines();
        Self {
            rope,
            line_endings: vec![None; line_count],
        }
    }

    /// Get the number of lines in the text.
    #[inline]
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get the total length in bytes.
    #[inline]
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Check if the text is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Get a line by index as a rope slice.
    ///
    /// Returns `None` if the line index is out of bounds.
    #[inline]
    pub fn line(&self, line_idx: usize) -> Option<RopeSlice<'_>> {
        if line_idx < self.rope.len_lines() {
            Some(self.rope.line(line_idx))
        } else {
            None
        }
    }

    /// Get the text of a line as a `Cow<str>`.
    ///
    /// This is efficient for lines that are stored contiguously,
    /// returning a borrowed reference. For lines that span chunk
    /// boundaries, it allocates a new String.
    pub fn line_text(&self, line_idx: usize) -> Option<Cow<'_, str>> {
        let line = self.line(line_idx)?;
        // Get the line without the trailing newline
        let text = line_without_ending(line);
        Some(text)
    }

    /// Get the byte offset of the start of a line.
    #[inline]
    pub fn line_to_byte(&self, line_idx: usize) -> usize {
        self.rope.line_to_byte(line_idx)
    }

    /// Get the line index for a byte offset.
    #[inline]
    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_line(byte_idx)
    }

    /// Get the byte offset of the start of a line, returning None if out of bounds.
    pub fn try_line_to_byte(&self, line_idx: usize) -> Option<usize> {
        if line_idx <= self.rope.len_lines() {
            Some(self.rope.line_to_byte(line_idx))
        } else {
            None
        }
    }

    /// Get the char offset of the start of a line.
    #[inline]
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    /// Get the line index for a char offset.
    #[inline]
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    /// Replace all text with new content.
    pub fn set_text(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.line_endings = vec![None; self.rope.len_lines()];
    }

    /// Insert text at a byte offset.
    pub fn insert(&mut self, byte_offset: usize, text: &str) {
        let char_offset = self.rope.byte_to_char(byte_offset);
        let line_before = self.rope.char_to_line(char_offset);
        self.rope.insert(char_offset, text);

        // Update line endings cache
        let new_line_count = self.rope.len_lines();
        let lines_added = new_line_count.saturating_sub(self.line_endings.len());
        if lines_added > 0 {
            // Insert None entries for new lines
            let insert_pos = line_before + 1;
            for _ in 0..lines_added {
                if insert_pos < self.line_endings.len() {
                    self.line_endings.insert(insert_pos, None);
                } else {
                    self.line_endings.push(None);
                }
            }
        }
        // Invalidate the line we inserted into
        if line_before < self.line_endings.len() {
            self.line_endings[line_before] = None;
        }
    }

    /// Delete a byte range.
    pub fn delete(&mut self, start_byte: usize, end_byte: usize) {
        if start_byte >= end_byte {
            return;
        }

        let start_char = self.rope.byte_to_char(start_byte);
        let end_char = self.rope.byte_to_char(end_byte);
        let start_line = self.rope.char_to_line(start_char);
        let end_line = self.rope.char_to_line(end_char.saturating_sub(1));

        self.rope.remove(start_char..end_char);

        // Update line endings cache
        let new_line_count = self.rope.len_lines();
        if end_line > start_line {
            // Remove entries for deleted lines
            let remove_count = end_line - start_line;
            for _ in 0..remove_count {
                if start_line + 1 < self.line_endings.len() {
                    self.line_endings.remove(start_line + 1);
                }
            }
        }
        // Truncate if we have too many
        self.line_endings.truncate(new_line_count);
        // Invalidate the line we deleted from
        if start_line < self.line_endings.len() {
            self.line_endings[start_line] = None;
        }
    }

    /// Get a slice of the rope as a string.
    pub fn slice(&self, start_byte: usize, end_byte: usize) -> Cow<'_, str> {
        let start_char = self.rope.byte_to_char(start_byte);
        let end_char = self.rope.byte_to_char(end_byte);
        let slice = self.rope.slice(start_char..end_char);
        slice_to_cow(slice)
    }

    /// Get the underlying rope.
    #[inline]
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Detect the line ending for a specific line.
    pub fn detect_line_ending(&self, line_idx: usize) -> LineEnding {
        if let Some(line) = self.line(line_idx) {
            detect_line_ending_from_slice(line)
        } else {
            LineEnding::None
        }
    }

    /// Get or detect the line ending for a line, caching the result.
    pub fn line_ending(&mut self, line_idx: usize) -> LineEnding {
        // Ensure we have enough entries
        while self.line_endings.len() <= line_idx {
            self.line_endings.push(None);
        }

        if let Some(ending) = self.line_endings[line_idx] {
            ending
        } else {
            let ending = self.detect_line_ending(line_idx);
            self.line_endings[line_idx] = Some(ending);
            ending
        }
    }

    /// Set the line ending for a specific line (used when editing).
    pub fn set_line_ending(&mut self, line_idx: usize, ending: LineEnding) {
        while self.line_endings.len() <= line_idx {
            self.line_endings.push(None);
        }
        self.line_endings[line_idx] = Some(ending);
    }

    /// Clear all cached line endings.
    pub fn clear_line_endings_cache(&mut self) {
        for ending in &mut self.line_endings {
            *ending = None;
        }
    }

    /// Convert the entire rope to a String.
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// Iterate over lines as `RopeSlice`.
    pub fn lines(&self) -> impl Iterator<Item = RopeSlice<'_>> {
        self.rope.lines()
    }
}

/// Convert a RopeSlice to Cow<str>, borrowing if contiguous.
fn slice_to_cow(slice: RopeSlice<'_>) -> Cow<'_, str> {
    if let Some(s) = slice.as_str() {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(slice.to_string())
    }
}

/// Get line text without trailing line ending.
fn line_without_ending(line: RopeSlice<'_>) -> Cow<'_, str> {
    let text = slice_to_cow(line);
    // Remove trailing \r\n, \n, \r, or \n\r
    let trimmed = text
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .trim_end_matches('\n');
    if trimmed.len() == text.len() {
        text
    } else {
        Cow::Owned(trimmed.to_string())
    }
}

/// Detect line ending from a rope slice.
fn detect_line_ending_from_slice(line: RopeSlice<'_>) -> LineEnding {
    let len = line.len_bytes();
    if len == 0 {
        return LineEnding::None;
    }

    // Check last two characters
    let last_char = line.byte(len - 1);
    let second_last = if len >= 2 {
        Some(line.byte(len - 2))
    } else {
        None
    };

    match (second_last, last_char) {
        (Some(b'\r'), b'\n') => LineEnding::CrLf,
        (Some(b'\n'), b'\r') => LineEnding::LfCr,
        (_, b'\n') => LineEnding::Lf,
        (_, b'\r') => LineEnding::Cr,
        _ => LineEnding::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut rope = RopeText::from_str("Hello\nWorld\n");
        assert_eq!(rope.line_count(), 3);
        assert_eq!(rope.line_text(0).unwrap().as_ref(), "Hello");
        assert_eq!(rope.line_text(1).unwrap().as_ref(), "World");
        assert_eq!(rope.line_text(2).unwrap().as_ref(), "");
    }

    #[test]
    fn test_insert() {
        let mut rope = RopeText::from_str("Hello World");
        rope.insert(5, ",");
        assert_eq!(rope.to_string(), "Hello, World");
    }

    #[test]
    fn test_delete() {
        let mut rope = RopeText::from_str("Hello, World");
        rope.delete(5, 7);
        assert_eq!(rope.to_string(), "HelloWorld");
    }

    #[test]
    fn test_line_endings() {
        let mut rope = RopeText::from_str("Line1\nLine2\r\nLine3\rLine4");
        assert_eq!(rope.line_ending(0), LineEnding::Lf);
        assert_eq!(rope.line_ending(1), LineEnding::CrLf);
        assert_eq!(rope.line_ending(2), LineEnding::Cr);
        assert_eq!(rope.line_ending(3), LineEnding::None);
    }
}
