// SPDX-License-Identifier: MIT OR Apache-2.0

use unicode_script::Script;

// Fallbacks to use after any script specific fallbacks
pub fn common_fallback() -> &'static [&'static str] {
    &[]
}

// Fallbacks to never use
pub fn forbidden_fallback() -> &'static [&'static str] {
    &[]
}

// Fallbacks to use per script
pub fn script_fallback(_script: Script, _locale: &str) -> &'static [&'static str] {
    &[]
}
