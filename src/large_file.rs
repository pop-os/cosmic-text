// SPDX-License-Identifier: MIT OR Apache-2.0

//! Large file support utilities.
//!
//! This module provides utilities for efficiently loading and displaying
//! large files in cosmic-text. For files over a configurable threshold,
//! it uses the rope-based backend which is much more memory efficient.

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::{
    Attrs, Buffer, FontSystem, Metrics, RopeBuffer, Shaping,
};

/// Threshold in bytes above which to use RopeBuffer.
/// Default is 1MB.
pub const DEFAULT_LARGE_FILE_THRESHOLD: usize = 1024 * 1024;

/// Result of loading a file - either a standard Buffer or a RopeBuffer.
#[derive(Debug)]
pub enum LoadedBuffer {
    /// Standard buffer for small/medium files
    Standard(Buffer),
    /// Rope-based buffer for large files
    Rope(RopeBuffer),
}

impl LoadedBuffer {
    /// Load text into the appropriate buffer type based on size.
    ///
    /// For files smaller than `threshold` bytes, uses standard `Buffer`.
    /// For larger files, uses `RopeBuffer` which is more memory efficient.
    pub fn from_text(
        font_system: &mut FontSystem,
        text: &str,
        attrs: &Attrs,
        metrics: Metrics,
        shaping: Shaping,
        threshold: usize,
    ) -> Self {
        if text.len() > threshold {
            log::info!(
                "Using RopeBuffer for large file ({} bytes, {} lines)",
                text.len(),
                text.lines().count()
            );
            let mut buffer = RopeBuffer::new_empty(metrics);
            buffer.set_text(font_system, text, attrs, shaping, None);
            LoadedBuffer::Rope(buffer)
        } else {
            let mut buffer = Buffer::new_empty(metrics);
            buffer.set_text(font_system, text, attrs, shaping, None);
            LoadedBuffer::Standard(buffer)
        }
    }

    /// Load text with default threshold.
    pub fn from_text_auto(
        font_system: &mut FontSystem,
        text: &str,
        attrs: &Attrs,
        metrics: Metrics,
        shaping: Shaping,
    ) -> Self {
        Self::from_text(font_system, text, attrs, metrics, shaping, DEFAULT_LARGE_FILE_THRESHOLD)
    }

    /// Check if this is a rope-based buffer (large file).
    pub fn is_rope(&self) -> bool {
        matches!(self, LoadedBuffer::Rope(_))
    }

    /// Get the line count.
    pub fn line_count(&self) -> usize {
        match self {
            LoadedBuffer::Standard(b) => b.line_count(),
            LoadedBuffer::Rope(b) => b.line_count(),
        }
    }

    /// Get line text.
    pub fn line_text(&self, line_i: usize) -> Option<String> {
        match self {
            LoadedBuffer::Standard(b) => b.line_text(line_i).map(|s| s.to_string()),
            LoadedBuffer::Rope(b) => b.line_text(line_i),
        }
    }

    /// Set scroll position.
    pub fn set_scroll(&mut self, scroll: crate::Scroll) {
        match self {
            LoadedBuffer::Standard(b) => b.set_scroll(scroll),
            LoadedBuffer::Rope(b) => b.set_scroll(scroll),
        }
    }

    /// Get scroll position.
    pub fn scroll(&self) -> crate::Scroll {
        match self {
            LoadedBuffer::Standard(b) => b.scroll(),
            LoadedBuffer::Rope(b) => b.scroll(),
        }
    }

    /// Check if redraw is needed.
    pub fn redraw(&self) -> bool {
        match self {
            LoadedBuffer::Standard(b) => b.redraw(),
            LoadedBuffer::Rope(b) => b.redraw(),
        }
    }

    /// Set redraw flag.
    pub fn set_redraw(&mut self, redraw: bool) {
        match self {
            LoadedBuffer::Standard(b) => b.set_redraw(redraw),
            LoadedBuffer::Rope(b) => b.set_redraw(redraw),
        }
    }

    /// Set buffer size.
    pub fn set_size(&mut self, font_system: &mut FontSystem, width: Option<f32>, height: Option<f32>) {
        match self {
            LoadedBuffer::Standard(b) => b.set_size(font_system, width, height),
            LoadedBuffer::Rope(b) => b.set_size(font_system, width, height),
        }
    }

    /// Get buffer size.
    pub fn size(&self) -> (Option<f32>, Option<f32>) {
        match self {
            LoadedBuffer::Standard(b) => b.size(),
            LoadedBuffer::Rope(b) => b.size(),
        }
    }

    /// Shape lines until scroll.
    pub fn shape_until_scroll(&mut self, font_system: &mut FontSystem, prune: bool) {
        match self {
            LoadedBuffer::Standard(b) => b.shape_until_scroll(font_system, prune),
            LoadedBuffer::Rope(b) => b.shape_until_scroll(font_system, prune),
        }
    }

    /// Convert to standard Buffer if not already.
    ///
    /// Warning: This will allocate memory for all lines if converting from RopeBuffer.
    /// Only use this for files that are small enough to fit in memory as a standard Buffer.
    pub fn into_standard(self, font_system: &mut FontSystem, metrics: Metrics) -> Buffer {
        match self {
            LoadedBuffer::Standard(b) => b,
            LoadedBuffer::Rope(b) => b.to_buffer(font_system, metrics),
        }
    }

    /// Get as standard Buffer if it is one.
    pub fn as_standard(&self) -> Option<&Buffer> {
        match self {
            LoadedBuffer::Standard(b) => Some(b),
            LoadedBuffer::Rope(_) => None,
        }
    }

    /// Get as standard Buffer mutably if it is one.
    pub fn as_standard_mut(&mut self) -> Option<&mut Buffer> {
        match self {
            LoadedBuffer::Standard(b) => Some(b),
            LoadedBuffer::Rope(_) => None,
        }
    }

    /// Get as RopeBuffer if it is one.
    pub fn as_rope(&self) -> Option<&RopeBuffer> {
        match self {
            LoadedBuffer::Standard(_) => None,
            LoadedBuffer::Rope(b) => Some(b),
        }
    }

    /// Get as RopeBuffer mutably if it is one.
    pub fn as_rope_mut(&mut self) -> Option<&mut RopeBuffer> {
        match self {
            LoadedBuffer::Standard(_) => None,
            LoadedBuffer::Rope(b) => Some(b),
        }
    }
}

/// Check if a file should be loaded with rope-based buffer.
pub fn should_use_rope(file_size: usize) -> bool {
    file_size > DEFAULT_LARGE_FILE_THRESHOLD
}

/// Check if a file should be loaded with rope-based buffer with custom threshold.
pub fn should_use_rope_with_threshold(file_size: usize, threshold: usize) -> bool {
    file_size > threshold
}
