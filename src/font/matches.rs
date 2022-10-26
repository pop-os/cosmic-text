// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use crate::Font;

/// Fonts that match a pattern
pub struct FontMatches<'a> {
    pub locale: &'a str,
    pub default_family: String,
    pub fonts: Vec<Arc<Font<'a>>>,
}
