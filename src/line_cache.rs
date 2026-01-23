// SPDX-License-Identifier: MIT OR Apache-2.0

//! LRU cache for line shaping and layout results.
//!
//! This module provides efficient caching of expensive shaping and layout
//! computations for visible lines only, instead of storing results for
//! all lines in the document.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::num::NonZeroUsize;
use lru::LruCache;

use crate::{LayoutLine, ShapeLine};

/// Default cache size for shape/layout caches.
/// This should be large enough to hold all visible lines plus some buffer.
const DEFAULT_CACHE_SIZE: usize = 1000;

/// Cache for shaped lines.
#[derive(Debug)]
pub struct ShapeCache {
    cache: LruCache<usize, ShapeLine>,
}

impl Default for ShapeCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_SIZE)
    }
}

impl ShapeCache {
    /// Create a new shape cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(capacity.max(1)).expect("capacity must be > 0"),
            ),
        }
    }

    /// Get a shaped line from the cache.
    pub fn get(&mut self, line_idx: usize) -> Option<&ShapeLine> {
        self.cache.get(&line_idx)
    }

    /// Get a mutable reference to a shaped line from the cache.
    pub fn get_mut(&mut self, line_idx: usize) -> Option<&mut ShapeLine> {
        self.cache.get_mut(&line_idx)
    }

    /// Check if a line is in the cache without updating LRU order.
    pub fn contains(&self, line_idx: usize) -> bool {
        self.cache.contains(&line_idx)
    }

    /// Insert a shaped line into the cache.
    pub fn insert(&mut self, line_idx: usize, shape: ShapeLine) {
        self.cache.put(line_idx, shape);
    }

    /// Remove a shaped line from the cache.
    pub fn remove(&mut self, line_idx: usize) -> Option<ShapeLine> {
        self.cache.pop(&line_idx)
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Invalidate all lines at or after the given index.
    ///
    /// This is used when text is inserted or deleted, which invalidates
    /// all subsequent line shapes.
    pub fn invalidate_from(&mut self, start_line: usize) {
        // LruCache doesn't have a nice way to do this, so we collect and remove
        let to_remove: Vec<_> = self
            .cache
            .iter()
            .filter_map(|(k, _)| if *k >= start_line { Some(*k) } else { None })
            .collect();
        for key in to_remove {
            self.cache.pop(&key);
        }
    }

    /// Shift all line indices at or after `start` by `delta`.
    ///
    /// Used when lines are inserted (positive delta) or removed (negative delta).
    pub fn shift_lines(&mut self, start: usize, delta: isize) {
        if delta == 0 {
            return;
        }

        // Collect all entries that need to be shifted
        let entries: Vec<_> = self
            .cache
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        self.cache.clear();

        for (idx, shape) in entries {
            if idx >= start {
                let new_idx = if delta > 0 {
                    idx.checked_add(delta as usize)
                } else {
                    idx.checked_sub((-delta) as usize)
                };
                if let Some(new_idx) = new_idx {
                    self.cache.put(new_idx, shape);
                }
            } else {
                self.cache.put(idx, shape);
            }
        }
    }

    /// Resize the cache capacity.
    pub fn resize(&mut self, capacity: usize) {
        self.cache
            .resize(NonZeroUsize::new(capacity.max(1)).expect("capacity must be > 0"));
    }
}

/// Cache for laid out lines.
#[derive(Debug)]
pub struct LayoutCache {
    cache: LruCache<usize, Vec<LayoutLine>>,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_SIZE)
    }
}

impl LayoutCache {
    /// Create a new layout cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(capacity.max(1)).expect("capacity must be > 0"),
            ),
        }
    }

    /// Get a laid out line from the cache.
    pub fn get(&mut self, line_idx: usize) -> Option<&Vec<LayoutLine>> {
        self.cache.get(&line_idx)
    }

    /// Get a mutable reference to a laid out line from the cache.
    pub fn get_mut(&mut self, line_idx: usize) -> Option<&mut Vec<LayoutLine>> {
        self.cache.get_mut(&line_idx)
    }

    /// Check if a line is in the cache without updating LRU order.
    pub fn contains(&self, line_idx: usize) -> bool {
        self.cache.contains(&line_idx)
    }

    /// Insert a laid out line into the cache.
    pub fn insert(&mut self, line_idx: usize, layout: Vec<LayoutLine>) {
        self.cache.put(line_idx, layout);
    }

    /// Remove a laid out line from the cache.
    pub fn remove(&mut self, line_idx: usize) -> Option<Vec<LayoutLine>> {
        self.cache.pop(&line_idx)
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Invalidate all lines at or after the given index.
    pub fn invalidate_from(&mut self, start_line: usize) {
        let to_remove: Vec<_> = self
            .cache
            .iter()
            .filter_map(|(k, _)| if *k >= start_line { Some(*k) } else { None })
            .collect();
        for key in to_remove {
            self.cache.pop(&key);
        }
    }

    /// Shift all line indices at or after `start` by `delta`.
    pub fn shift_lines(&mut self, start: usize, delta: isize) {
        if delta == 0 {
            return;
        }

        let entries: Vec<_> = self
            .cache
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        self.cache.clear();

        for (idx, layout) in entries {
            if idx >= start {
                let new_idx = if delta > 0 {
                    idx.checked_add(delta as usize)
                } else {
                    idx.checked_sub((-delta) as usize)
                };
                if let Some(new_idx) = new_idx {
                    self.cache.put(new_idx, layout);
                }
            } else {
                self.cache.put(idx, layout);
            }
        }
    }

    /// Resize the cache capacity.
    pub fn resize(&mut self, capacity: usize) {
        self.cache
            .resize(NonZeroUsize::new(capacity.max(1)).expect("capacity must be > 0"));
    }
}

/// Combined cache for both shaping and layout.
#[derive(Debug, Default)]
pub struct LineCache {
    /// Cache for shaped lines.
    pub shape: ShapeCache,
    /// Cache for laid out lines.
    pub layout: LayoutCache,
}

impl LineCache {
    /// Create a new line cache with the given capacity for each cache.
    pub fn new(capacity: usize) -> Self {
        Self {
            shape: ShapeCache::new(capacity),
            layout: LayoutCache::new(capacity),
        }
    }

    /// Clear both caches.
    pub fn clear(&mut self) {
        self.shape.clear();
        self.layout.clear();
    }

    /// Invalidate a specific line in both caches.
    pub fn invalidate_line(&mut self, line_idx: usize) {
        self.shape.remove(line_idx);
        self.layout.remove(line_idx);
    }

    /// Invalidate all lines at or after the given index in both caches.
    pub fn invalidate_from(&mut self, start_line: usize) {
        self.shape.invalidate_from(start_line);
        self.layout.invalidate_from(start_line);
    }

    /// Shift all line indices in both caches.
    pub fn shift_lines(&mut self, start: usize, delta: isize) {
        self.shape.shift_lines(start, delta);
        self.layout.shift_lines(start, delta);
    }

    /// Resize both caches.
    pub fn resize(&mut self, capacity: usize) {
        self.shape.resize(capacity);
        self.layout.resize(capacity);
    }

    /// Invalidate only the layout cache for a line (shape is still valid).
    pub fn invalidate_layout(&mut self, line_idx: usize) {
        self.layout.remove(line_idx);
    }

    /// Invalidate all layout entries (when layout parameters change).
    pub fn clear_layout(&mut self) {
        self.layout.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_cache_basic() {
        let mut cache = ShapeCache::new(10);
        assert!(cache.is_empty());

        let shape = ShapeLine::empty();
        cache.insert(5, shape);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(5));
        assert!(!cache.contains(0));

        cache.remove(5);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_shift_lines() {
        let mut cache = ShapeCache::new(10);
        cache.insert(0, ShapeLine::empty());
        cache.insert(5, ShapeLine::empty());
        cache.insert(10, ShapeLine::empty());

        // Insert 2 lines at position 3
        cache.shift_lines(3, 2);

        assert!(cache.contains(0)); // Unchanged
        assert!(!cache.contains(5)); // Moved
        assert!(cache.contains(7)); // Was 5
        assert!(!cache.contains(10)); // Moved
        assert!(cache.contains(12)); // Was 10
    }

    #[test]
    fn test_invalidate_from() {
        let mut cache = ShapeCache::new(10);
        cache.insert(0, ShapeLine::empty());
        cache.insert(5, ShapeLine::empty());
        cache.insert(10, ShapeLine::empty());

        cache.invalidate_from(5);

        assert!(cache.contains(0));
        assert!(!cache.contains(5));
        assert!(!cache.contains(10));
    }
}
