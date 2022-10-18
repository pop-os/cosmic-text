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
pub fn script_fallback(script: &Script, locale: &str) -> &'static [&'static str] {
    &[]
}
