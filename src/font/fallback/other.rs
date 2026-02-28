// SPDX-License-Identifier: MIT OR Apache-2.0

use super::Fallback;
use core::default::Default;

/// An empty platform-specific font fallback list.
pub fn platform_fallback(_locale: &str) -> Fallback {
    Fallback {
        common_fallback: Default::default(),
        forbidden_fallback: Default::default(),
        script_fallback: Default::default(),
    }
}
