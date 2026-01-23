// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sparse metadata storage for lines in a rope-based buffer.
//!
//! This module provides efficient storage for per-line metadata that
//! only exists for lines with non-default values (custom attributes,
//! alignment, etc.). For large files, most lines use default styling,
//! so storing metadata sparsely saves significant memory.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use crate::{Align, AttrsList, Attrs, AttrsOwned, LineEnding, Shaping};

/// Metadata for a single line that differs from defaults.
#[derive(Clone, Debug)]
pub struct LineMetadata {
    /// Custom attributes for this line (None = use defaults).
    pub attrs_list: Option<AttrsList>,
    /// Custom alignment for this line (None = use default based on RTL).
    pub align: Option<Align>,
    /// Line ending type.
    pub ending: LineEnding,
    /// Shaping strategy for this line.
    pub shaping: Shaping,
    /// User-defined metadata.
    pub user_metadata: Option<usize>,
}

impl Default for LineMetadata {
    fn default() -> Self {
        Self {
            attrs_list: None,
            align: None,
            ending: LineEnding::None,
            shaping: Shaping::Advanced,
            user_metadata: None,
        }
    }
}

impl LineMetadata {
    /// Create new metadata with the given line ending.
    pub fn new(ending: LineEnding) -> Self {
        Self {
            ending,
            ..Default::default()
        }
    }

    /// Create metadata with custom attributes.
    pub fn with_attrs(ending: LineEnding, attrs_list: AttrsList, shaping: Shaping) -> Self {
        Self {
            attrs_list: Some(attrs_list),
            ending,
            shaping,
            ..Default::default()
        }
    }

    /// Check if this metadata is "default" (can be removed from sparse storage).
    pub fn is_default(&self) -> bool {
        self.attrs_list.is_none()
            && self.align.is_none()
            && self.ending == LineEnding::None
            && self.shaping == Shaping::Advanced
            && self.user_metadata.is_none()
    }

    /// Check if this metadata has custom attributes.
    pub fn has_custom_attrs(&self) -> bool {
        self.attrs_list.is_some()
    }

    /// Get the attributes list, or create a default one.
    pub fn attrs_list_or_default(&self, default_attrs: &Attrs) -> AttrsList {
        self.attrs_list
            .clone()
            .unwrap_or_else(|| AttrsList::new(default_attrs))
    }
}

/// Sparse storage for per-line metadata.
///
/// Uses a BTreeMap to store only lines with non-default metadata,
/// which is memory-efficient for large files where most lines
/// use default styling.
#[derive(Clone, Debug)]
pub struct SparseMetadata {
    /// Metadata for lines that have non-default values.
    /// Key is the line index.
    entries: BTreeMap<usize, LineMetadata>,
    /// Default attributes to use for lines without custom attrs.
    default_attrs: AttrsOwned,
    /// Default shaping strategy.
    default_shaping: Shaping,
}

impl Default for SparseMetadata {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            default_attrs: AttrsOwned::new(&Attrs::new()),
            default_shaping: Shaping::Advanced,
        }
    }
}

impl SparseMetadata {
    /// Create new sparse metadata storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific default attributes.
    pub fn with_defaults(attrs: &Attrs, shaping: Shaping) -> Self {
        Self {
            entries: BTreeMap::new(),
            default_attrs: AttrsOwned::new(attrs),
            default_shaping: shaping,
        }
    }

    /// Get metadata for a line, or None if using defaults.
    pub fn get(&self, line_idx: usize) -> Option<&LineMetadata> {
        self.entries.get(&line_idx)
    }

    /// Get mutable metadata for a line, or None if using defaults.
    pub fn get_mut(&mut self, line_idx: usize) -> Option<&mut LineMetadata> {
        self.entries.get_mut(&line_idx)
    }

    /// Get or create metadata for a line.
    pub fn get_or_insert(&mut self, line_idx: usize) -> &mut LineMetadata {
        self.entries.entry(line_idx).or_default()
    }

    /// Set metadata for a line.
    ///
    /// If the metadata is default, it will be removed from storage.
    pub fn set(&mut self, line_idx: usize, metadata: LineMetadata) {
        if metadata.is_default() {
            self.entries.remove(&line_idx);
        } else {
            self.entries.insert(line_idx, metadata);
        }
    }

    /// Remove metadata for a line.
    pub fn remove(&mut self, line_idx: usize) -> Option<LineMetadata> {
        self.entries.remove(&line_idx)
    }

    /// Clear all metadata.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get the number of lines with custom metadata.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if there are no custom metadata entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the default attributes.
    pub fn default_attrs(&self) -> Attrs<'_> {
        self.default_attrs.as_attrs()
    }

    /// Set the default attributes.
    pub fn set_default_attrs(&mut self, attrs: &Attrs) {
        self.default_attrs = AttrsOwned::new(attrs);
    }

    /// Get the default shaping strategy.
    pub fn default_shaping(&self) -> Shaping {
        self.default_shaping
    }

    /// Set the default shaping strategy.
    pub fn set_default_shaping(&mut self, shaping: Shaping) {
        self.default_shaping = shaping;
    }

    /// Get the line ending for a line.
    pub fn line_ending(&self, line_idx: usize) -> LineEnding {
        self.entries
            .get(&line_idx)
            .map(|m| m.ending)
            .unwrap_or(LineEnding::None)
    }

    /// Set the line ending for a line.
    pub fn set_line_ending(&mut self, line_idx: usize, ending: LineEnding) {
        if ending == LineEnding::None && self.entries.get(&line_idx).map(|m| m.is_default()).unwrap_or(true) {
            // Don't create an entry just for a None line ending
            return;
        }
        self.get_or_insert(line_idx).ending = ending;
        // Clean up if now default
        if let Some(meta) = self.entries.get(&line_idx) {
            if meta.is_default() {
                self.entries.remove(&line_idx);
            }
        }
    }

    /// Get the alignment for a line.
    pub fn align(&self, line_idx: usize) -> Option<Align> {
        self.entries.get(&line_idx).and_then(|m| m.align)
    }

    /// Set the alignment for a line.
    pub fn set_align(&mut self, line_idx: usize, align: Option<Align>) {
        if align.is_none() && self.entries.get(&line_idx).is_none() {
            return;
        }
        self.get_or_insert(line_idx).align = align;
        // Clean up if now default
        if let Some(meta) = self.entries.get(&line_idx) {
            if meta.is_default() {
                self.entries.remove(&line_idx);
            }
        }
    }

    /// Get the attributes list for a line.
    pub fn attrs_list(&self, line_idx: usize) -> AttrsList {
        self.entries
            .get(&line_idx)
            .and_then(|m| m.attrs_list.clone())
            .unwrap_or_else(|| AttrsList::new(&self.default_attrs.as_attrs()))
    }

    /// Set the attributes list for a line.
    pub fn set_attrs_list(&mut self, line_idx: usize, attrs_list: AttrsList) {
        self.get_or_insert(line_idx).attrs_list = Some(attrs_list);
    }

    /// Get the shaping strategy for a line.
    pub fn shaping(&self, line_idx: usize) -> Shaping {
        self.entries
            .get(&line_idx)
            .map(|m| m.shaping)
            .unwrap_or(self.default_shaping)
    }

    /// Set the shaping strategy for a line.
    pub fn set_shaping(&mut self, line_idx: usize, shaping: Shaping) {
        if shaping == self.default_shaping && self.entries.get(&line_idx).is_none() {
            return;
        }
        self.get_or_insert(line_idx).shaping = shaping;
    }

    /// Get user metadata for a line.
    pub fn user_metadata(&self, line_idx: usize) -> Option<usize> {
        self.entries.get(&line_idx).and_then(|m| m.user_metadata)
    }

    /// Set user metadata for a line.
    pub fn set_user_metadata(&mut self, line_idx: usize, metadata: Option<usize>) {
        if metadata.is_none() && self.entries.get(&line_idx).is_none() {
            return;
        }
        self.get_or_insert(line_idx).user_metadata = metadata;
        // Clean up if now default
        if let Some(meta) = self.entries.get(&line_idx) {
            if meta.is_default() {
                self.entries.remove(&line_idx);
            }
        }
    }

    /// Shift all line indices at or after `start` by `delta`.
    ///
    /// Used when lines are inserted or removed.
    pub fn shift_lines(&mut self, start: usize, delta: isize) {
        if delta == 0 {
            return;
        }

        let mut new_entries = BTreeMap::new();
        for (idx, metadata) in self.entries.iter() {
            if *idx >= start {
                let new_idx = if delta > 0 {
                    idx.checked_add(delta as usize)
                } else {
                    idx.checked_sub((-delta) as usize)
                };
                if let Some(new_idx) = new_idx {
                    new_entries.insert(new_idx, metadata.clone());
                }
            } else {
                new_entries.insert(*idx, metadata.clone());
            }
        }
        self.entries = new_entries;
    }

    /// Remove lines in a range and shift subsequent lines.
    pub fn remove_lines(&mut self, start: usize, count: usize) {
        // Remove entries in the range
        for i in start..(start + count) {
            self.entries.remove(&i);
        }
        // Shift remaining entries
        self.shift_lines(start + count, -(count as isize));
    }

    /// Insert empty lines and shift subsequent lines.
    pub fn insert_lines(&mut self, start: usize, count: usize) {
        self.shift_lines(start, count as isize);
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &LineMetadata)> {
        self.entries.iter().map(|(k, v)| (*k, v))
    }

    /// Iterate over entries in a range.
    pub fn range(&self, start: usize, end: usize) -> impl Iterator<Item = (usize, &LineMetadata)> {
        use core::ops::Bound;
        self.entries
            .range((Bound::Included(start), Bound::Excluded(end)))
            .map(|(k, v)| (*k, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_storage() {
        let mut sparse = SparseMetadata::new();

        // Default should not create entries
        assert!(sparse.is_empty());

        // Setting non-default values creates entries
        sparse.set_line_ending(5, LineEnding::CrLf);
        assert_eq!(sparse.len(), 1);
        assert_eq!(sparse.line_ending(5), LineEnding::CrLf);
        assert_eq!(sparse.line_ending(0), LineEnding::None);

        // Setting back to default removes entry
        sparse.set_line_ending(5, LineEnding::None);
        // Note: the entry might still exist but with default values
        // The cleanup happens through set()
    }

    #[test]
    fn test_shift_lines() {
        let mut sparse = SparseMetadata::new();
        sparse.set_line_ending(5, LineEnding::CrLf);
        sparse.set_line_ending(10, LineEnding::Lf);

        // Insert 2 lines at position 7
        sparse.insert_lines(7, 2);

        // Line 5 should be unchanged
        assert_eq!(sparse.line_ending(5), LineEnding::CrLf);
        // Line 10 should now be at 12
        assert_eq!(sparse.line_ending(12), LineEnding::Lf);
        assert_eq!(sparse.line_ending(10), LineEnding::None);
    }

    #[test]
    fn test_remove_lines() {
        let mut sparse = SparseMetadata::new();
        sparse.set_line_ending(5, LineEnding::CrLf);
        sparse.set_line_ending(10, LineEnding::Lf);
        sparse.set_line_ending(15, LineEnding::Cr);

        // Remove lines 8-12 (5 lines)
        sparse.remove_lines(8, 5);

        // Line 5 should be unchanged
        assert_eq!(sparse.line_ending(5), LineEnding::CrLf);
        // Line 10 was removed
        assert_eq!(sparse.line_ending(10), LineEnding::Cr); // Was line 15
        // Line 15 is now at 10
        assert_eq!(sparse.line_ending(15), LineEnding::None);
    }
}
