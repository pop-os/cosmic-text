// SPDX-License-Identifier: MIT OR Apache-2.0

//! Line view facade for rope-based buffer access.
//!
//! This module provides a `LineView` type that presents a BufferLine-like
//! interface for lines stored in a rope, allowing gradual migration of
//! code from direct Vec<BufferLine> access to the rope-based backend.

#[cfg(not(feature = "std"))]
use alloc::{borrow::Cow, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::borrow::Cow;

use crate::{
    Align, AttrsList, FontSystem, Hinting, LayoutLine, LineEnding, ShapeLine, Shaping, Wrap,
};

use crate::{LineCache, RopeText, SparseMetadata};

/// A view into a line stored in a rope-based buffer.
///
/// This provides a similar interface to `BufferLine` but works with
/// rope-based text storage and sparse metadata.
#[derive(Debug)]
pub struct LineView<'a> {
    /// The line index in the buffer.
    line_idx: usize,
    /// Reference to the rope text storage.
    rope: &'a RopeText,
    /// Reference to sparse metadata.
    metadata: &'a SparseMetadata,
    /// Reference to the line cache (for shape/layout).
    /// Note: Currently unused but reserved for future cache access through LineView.
    #[allow(dead_code)]
    cache: &'a LineCache,
}

impl<'a> LineView<'a> {
    /// Create a new line view.
    pub fn new(
        line_idx: usize,
        rope: &'a RopeText,
        metadata: &'a SparseMetadata,
        cache: &'a LineCache,
    ) -> Self {
        Self {
            line_idx,
            rope,
            metadata,
            cache,
        }
    }

    /// Get the line index.
    pub fn line_idx(&self) -> usize {
        self.line_idx
    }

    /// Get the text of this line as a borrowed string where possible.
    pub fn text(&self) -> Cow<'_, str> {
        self.rope.line_text(self.line_idx).unwrap_or(Cow::Borrowed(""))
    }

    /// Get the text of this line, allocating if necessary.
    pub fn text_owned(&self) -> String {
        self.text().into_owned()
    }

    /// Get the line ending.
    pub fn ending(&self) -> LineEnding {
        self.metadata.line_ending(self.line_idx)
    }

    /// Get the attributes list for this line.
    pub fn attrs_list(&self) -> AttrsList {
        self.metadata.attrs_list(self.line_idx)
    }

    /// Get the text alignment.
    pub fn align(&self) -> Option<Align> {
        self.metadata.align(self.line_idx)
    }

    /// Get the shaping strategy.
    pub fn shaping(&self) -> Shaping {
        self.metadata.shaping(self.line_idx)
    }

    /// Get the cached shape result, if available.
    pub fn shape_opt(&self) -> Option<&ShapeLine> {
        // Note: This requires interior mutability or a mutable cache reference
        // For now, we can't provide this without changing the API
        // The shape cache is accessed directly through LineViewMut
        None
    }

    /// Get the cached layout result, if available.
    pub fn layout_opt(&self) -> Option<&Vec<LayoutLine>> {
        // Same as shape_opt - requires mutable access to cache
        None
    }

    /// Get user metadata.
    pub fn metadata(&self) -> Option<usize> {
        self.metadata.user_metadata(self.line_idx)
    }
}

/// A mutable view into a line stored in a rope-based buffer.
///
/// This provides mutable access to line properties and caches.
#[derive(Debug)]
pub struct LineViewMut<'a> {
    /// The line index in the buffer.
    line_idx: usize,
    /// Mutable reference to the rope text storage.
    rope: &'a mut RopeText,
    /// Mutable reference to sparse metadata.
    metadata: &'a mut SparseMetadata,
    /// Mutable reference to the line cache.
    cache: &'a mut LineCache,
    /// Tab width for shaping.
    tab_width: u16,
    /// Font size for layout.
    font_size: f32,
    /// Width for layout.
    width_opt: Option<f32>,
    /// Wrap mode for layout.
    wrap: Wrap,
    /// Monospace width for layout.
    monospace_width: Option<f32>,
    /// Hinting strategy for layout.
    hinting: Hinting,
}

impl<'a> LineViewMut<'a> {
    /// Create a new mutable line view.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line_idx: usize,
        rope: &'a mut RopeText,
        metadata: &'a mut SparseMetadata,
        cache: &'a mut LineCache,
        tab_width: u16,
        font_size: f32,
        width_opt: Option<f32>,
        wrap: Wrap,
        monospace_width: Option<f32>,
        hinting: Hinting,
    ) -> Self {
        Self {
            line_idx,
            rope,
            metadata,
            cache,
            tab_width,
            font_size,
            width_opt,
            wrap,
            monospace_width,
            hinting,
        }
    }

    /// Get the line index.
    pub fn line_idx(&self) -> usize {
        self.line_idx
    }

    /// Get the text of this line.
    pub fn text(&self) -> Cow<'_, str> {
        self.rope.line_text(self.line_idx).unwrap_or(Cow::Borrowed(""))
    }

    /// Get the line ending.
    pub fn ending(&self) -> LineEnding {
        self.metadata.line_ending(self.line_idx)
    }

    /// Set the line ending.
    pub fn set_ending(&mut self, ending: LineEnding) -> bool {
        let old = self.ending();
        if ending != old {
            self.metadata.set_line_ending(self.line_idx, ending);
            self.reset_shaping();
            true
        } else {
            false
        }
    }

    /// Get the attributes list.
    pub fn attrs_list(&self) -> AttrsList {
        self.metadata.attrs_list(self.line_idx)
    }

    /// Set the attributes list.
    pub fn set_attrs_list(&mut self, attrs_list: AttrsList) -> bool {
        let old = self.attrs_list();
        if attrs_list != old {
            self.metadata.set_attrs_list(self.line_idx, attrs_list);
            self.reset_shaping();
            true
        } else {
            false
        }
    }

    /// Get the text alignment.
    pub fn align(&self) -> Option<Align> {
        self.metadata.align(self.line_idx)
    }

    /// Set the text alignment.
    pub fn set_align(&mut self, align: Option<Align>) -> bool {
        let old = self.align();
        if align != old {
            self.metadata.set_align(self.line_idx, align);
            self.reset_layout();
            true
        } else {
            false
        }
    }

    /// Get the shaping strategy.
    pub fn shaping(&self) -> Shaping {
        self.metadata.shaping(self.line_idx)
    }

    /// Get user metadata.
    pub fn metadata_user(&self) -> Option<usize> {
        self.metadata.user_metadata(self.line_idx)
    }

    /// Set user metadata.
    pub fn set_metadata(&mut self, user_metadata: usize) {
        self.metadata.set_user_metadata(self.line_idx, Some(user_metadata));
    }

    /// Reset all caches for this line.
    pub fn reset(&mut self) {
        self.metadata.set_user_metadata(self.line_idx, None);
        self.reset_shaping();
    }

    /// Reset shaping and layout caches for this line.
    pub fn reset_shaping(&mut self) {
        self.cache.shape.remove(self.line_idx);
        self.reset_layout();
    }

    /// Reset only layout cache for this line.
    pub fn reset_layout(&mut self) {
        self.cache.layout.remove(self.line_idx);
    }

    /// Shape this line, caching the result.
    pub fn shape(&mut self, font_system: &mut FontSystem) -> &ShapeLine {
        if !self.cache.shape.contains(self.line_idx) {
            let text = self.text().into_owned();
            let attrs_list = self.attrs_list();
            let shaping = self.shaping();

            let mut shape_line = ShapeLine::empty();
            shape_line.build(font_system, &text, &attrs_list, shaping, self.tab_width);
            self.cache.shape.insert(self.line_idx, shape_line);
            self.cache.layout.remove(self.line_idx);
        }
        self.cache.shape.get(self.line_idx).expect("shape not found after insert")
    }

    /// Get the cached shape, if available.
    pub fn shape_opt(&mut self) -> Option<&ShapeLine> {
        self.cache.shape.get(self.line_idx)
    }

    /// Layout this line, caching the result.
    pub fn layout(&mut self, font_system: &mut FontSystem) -> &[LayoutLine] {
        // First ensure we have a shape
        if !self.cache.shape.contains(self.line_idx) {
            self.shape(font_system);
        }

        if !self.cache.layout.contains(self.line_idx) {
            let align = self.align();
            let shape = self.cache.shape.get(self.line_idx).expect("shape must exist");

            let mut layout = Vec::with_capacity(1);
            shape.layout_to_buffer(
                &mut font_system.shape_buffer,
                self.font_size,
                self.width_opt,
                self.wrap,
                align,
                &mut layout,
                self.monospace_width,
                self.hinting,
            );
            self.cache.layout.insert(self.line_idx, layout);
        }
        self.cache.layout.get(self.line_idx).map(|v| v.as_slice()).expect("layout not found after insert")
    }

    /// Get the cached layout, if available.
    pub fn layout_opt(&mut self) -> Option<&[LayoutLine]> {
        self.cache.layout.get(self.line_idx).map(|v| v.as_slice())
    }
}

/// Iterator over line views in a rope-based buffer.
#[derive(Debug)]
pub struct LineViewIter<'a> {
    current: usize,
    end: usize,
    rope: &'a RopeText,
    metadata: &'a SparseMetadata,
    cache: &'a LineCache,
}

impl<'a> LineViewIter<'a> {
    /// Create a new iterator over all lines.
    pub fn new(rope: &'a RopeText, metadata: &'a SparseMetadata, cache: &'a LineCache) -> Self {
        Self {
            current: 0,
            end: rope.line_count(),
            rope,
            metadata,
            cache,
        }
    }

    /// Create a new iterator over a range of lines.
    pub fn range(
        start: usize,
        end: usize,
        rope: &'a RopeText,
        metadata: &'a SparseMetadata,
        cache: &'a LineCache,
    ) -> Self {
        Self {
            current: start,
            end: end.min(rope.line_count()),
            rope,
            metadata,
            cache,
        }
    }
}

impl<'a> Iterator for LineViewIter<'a> {
    type Item = LineView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let view = LineView::new(self.current, self.rope, self.metadata, self.cache);
            self.current += 1;
            Some(view)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end.saturating_sub(self.current);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for LineViewIter<'_> {}
