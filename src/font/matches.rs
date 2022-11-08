// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::{
    string::String,
    vec::Vec,
};

use crate::Font;

/// Fonts that match a pattern
pub struct FontMatches<'a> {
    pub locale: &'a str,
    pub default_family: String,
    pub fonts: Vec<Arc<Font<'a>>>,
}
