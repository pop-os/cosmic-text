// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use alloc::vec::Vec;
use fontdb::Family;
use unicode_script::Script;

use crate::{Font, FontMatchKey, FontSystem, ShapePlanCache};

use self::platform::*;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows",)))]
#[path = "other.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;

#[cfg(target_os = "linux")]
#[path = "unix.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

#[cfg(not(feature = "warn_on_missing_glyphs"))]
use log::debug as missing_warn;
#[cfg(feature = "warn_on_missing_glyphs")]
use log::warn as missing_warn;

// Match on lowest font_weight_diff, then script_non_matches, then font_weight
// Default font gets None for both `weight_offset` and `script_non_matches`, and thus, it is
// always the first to be popped from the set.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MonospaceFallbackInfo {
    font_weight_diff: Option<u16>,
    codepoint_non_matches: Option<usize>,
    font_weight: u16,
    id: fontdb::ID,
}

pub struct FontFallbackIter<'a> {
    font_system: &'a mut FontSystem,
    font_match_keys: &'a [FontMatchKey],
    default_families: &'a [&'a Family<'a>],
    monospace_fallbacks: BTreeSet<MonospaceFallbackInfo>,
    default_i: usize,
    scripts: &'a [Script],
    word: &'a str,
    script_i: (usize, usize),
    common_i: usize,
    other_i: usize,
    end: bool,
}

impl<'a> FontFallbackIter<'a> {
    pub fn new(
        font_system: &'a mut FontSystem,
        font_match_keys: &'a [FontMatchKey],
        default_families: &'a [&'a Family<'a>],
        scripts: &'a [Script],
        word: &'a str,
    ) -> Self {
        Self {
            font_system,
            font_match_keys,
            default_families,
            monospace_fallbacks: BTreeSet::new(),
            default_i: 0,
            scripts,
            word,
            script_i: (0, 0),
            common_i: 0,
            other_i: 0,
            end: false,
        }
    }

    pub fn check_missing(&mut self, word: &str) {
        if self.end {
            missing_warn!(
                "Failed to find any fallback for {:?} locale '{}': '{}'",
                self.scripts,
                self.font_system.locale(),
                word
            );
        } else if self.other_i > 0 {
            missing_warn!(
                "Failed to find preset fallback for {:?} locale '{}', used '{}': '{}'",
                self.scripts,
                self.font_system.locale(),
                self.face_name(self.font_match_keys[self.other_i - 1].id),
                word
            );
        } else if !self.scripts.is_empty() && self.common_i > 0 {
            let family = common_fallback()[self.common_i - 1];
            missing_warn!(
                "Failed to find script fallback for {:?} locale '{}', used '{}': '{}'",
                self.scripts,
                self.font_system.locale(),
                family,
                word
            );
        }
    }

    pub fn face_name(&self, id: fontdb::ID) -> &str {
        if let Some(face) = self.font_system.db().face(id) {
            if let Some((name, _)) = face.families.first() {
                name
            } else {
                &face.post_script_name
            }
        } else {
            "invalid font id"
        }
    }

    pub fn shape_plan_cache(&mut self) -> &mut ShapePlanCache {
        self.font_system.shape_plan_cache()
    }

    fn face_contains_family(&self, id: fontdb::ID, family_name: &str) -> bool {
        if let Some(face) = self.font_system.db().face(id) {
            face.families.iter().any(|(name, _)| name == family_name)
        } else {
            false
        }
    }

    fn default_font_match_key(&self) -> Option<&FontMatchKey> {
        let default_family = self.default_families[self.default_i - 1];
        let default_family_name = self.font_system.db().family_name(default_family);

        self.font_match_keys
            .iter()
            .filter(|m_key| m_key.font_weight_diff == 0)
            .find(|m_key| self.face_contains_family(m_key.id, default_family_name))
    }
}

impl<'a> Iterator for FontFallbackIter<'a> {
    type Item = Arc<Font>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(fallback_info) = self.monospace_fallbacks.pop_first() {
            if let Some(font) = self.font_system.get_font(fallback_info.id) {
                return Some(font);
            }
        }

        let font_match_keys_iter = |is_mono| {
            self.font_match_keys
                .iter()
                .filter(move |m_key| m_key.font_weight_diff == 0 || is_mono)
        };

        'DEF_FAM: while self.default_i < self.default_families.len() {
            self.default_i += 1;
            let is_mono = self.default_families[self.default_i - 1] == &Family::Monospace;
            let default_font_match_key = self.default_font_match_key().cloned();
            let word_chars_count = self.word.chars().count();

            macro_rules! mk_mono_fallback_info {
                ($m_key:expr) => {{
                    let supported_cp_count_opt = self
                        .font_system
                        .get_font_supported_codepoints_in_word($m_key.id, self.word);

                    supported_cp_count_opt.map(|supported_cp_count| {
                        let codepoint_non_matches = word_chars_count - supported_cp_count;

                        MonospaceFallbackInfo {
                            font_weight_diff: Some($m_key.font_weight_diff),
                            codepoint_non_matches: Some(codepoint_non_matches),
                            font_weight: $m_key.font_weight,
                            id: $m_key.id,
                        }
                    })
                }};
            }

            match (is_mono, default_font_match_key.as_ref()) {
                (false, None) => break 'DEF_FAM,
                (false, Some(m_key)) => {
                    if let Some(font) = self.font_system.get_font(m_key.id) {
                        return Some(font);
                    } else {
                        break 'DEF_FAM;
                    }
                }
                (true, None) => (),
                (true, Some(m_key)) => {
                    // Default Monospace font
                    if let Some(mut fallback_info) = mk_mono_fallback_info!(m_key) {
                        fallback_info.font_weight_diff = None;

                        // Return early if default Monospace font supports all word codepoints.
                        // Otherewise, add to fallbacks set
                        if fallback_info.codepoint_non_matches == Some(0) {
                            if let Some(font) = self.font_system.get_font(m_key.id) {
                                return Some(font);
                            }
                        } else {
                            assert!(self.monospace_fallbacks.insert(fallback_info));
                        }
                    }
                }
            };

            let mono_ids_for_scripts = if is_mono && !self.scripts.is_empty() {
                let scripts = self.scripts.iter().filter_map(|script| {
                    let script_as_lower = script.short_name().to_lowercase();
                    <[u8; 4]>::try_from(script_as_lower.as_bytes()).ok()
                });
                self.font_system.get_monospace_ids_for_scripts(scripts)
            } else {
                Vec::new()
            };

            for m_key in font_match_keys_iter(is_mono) {
                if Some(m_key.id) != default_font_match_key.as_ref().map(|m_key| m_key.id) {
                    let is_mono_id = if mono_ids_for_scripts.is_empty() {
                        self.font_system.is_monospace(m_key.id)
                    } else {
                        mono_ids_for_scripts.binary_search(&m_key.id).is_ok()
                    };

                    if is_mono_id {
                        let supported_cp_count_opt = self
                            .font_system
                            .get_font_supported_codepoints_in_word(m_key.id, self.word);
                        if let Some(supported_cp_count) = supported_cp_count_opt {
                            let codepoint_non_matches =
                                self.word.chars().count() - supported_cp_count;

                            let fallback_info = MonospaceFallbackInfo {
                                font_weight_diff: Some(m_key.font_weight_diff),
                                codepoint_non_matches: Some(codepoint_non_matches),
                                font_weight: m_key.font_weight,
                                id: m_key.id,
                            };
                            assert!(self.monospace_fallbacks.insert(fallback_info));
                        }
                    }
                }
            }
            // If default family is Monospace fallback to first monospaced font
            if let Some(fallback_info) = self.monospace_fallbacks.pop_first() {
                if let Some(font) = self.font_system.get_font(fallback_info.id) {
                    return Some(font);
                }
            }
        }

        while self.script_i.0 < self.scripts.len() {
            let script = self.scripts[self.script_i.0];

            let script_families = script_fallback(script, self.font_system.locale());
            while self.script_i.1 < script_families.len() {
                let script_family = script_families[self.script_i.1];
                self.script_i.1 += 1;
                for m_key in font_match_keys_iter(false) {
                    if self.face_contains_family(m_key.id, script_family) {
                        if let Some(font) = self.font_system.get_font(m_key.id) {
                            return Some(font);
                        }
                    }
                }
                log::debug!(
                    "failed to find family '{}' for script {:?} and locale '{}'",
                    script_family,
                    script,
                    self.font_system.locale(),
                );
            }

            self.script_i.0 += 1;
            self.script_i.1 = 0;
        }

        let common_families = common_fallback();
        while self.common_i < common_families.len() {
            let common_family = common_families[self.common_i];
            self.common_i += 1;
            for m_key in font_match_keys_iter(false) {
                if self.face_contains_family(m_key.id, common_family) {
                    if let Some(font) = self.font_system.get_font(m_key.id) {
                        return Some(font);
                    }
                }
            }
            log::debug!("failed to find family '{}'", common_family);
        }

        //TODO: do we need to do this?
        //TODO: do not evaluate fonts more than once!
        let forbidden_families = forbidden_fallback();
        while self.other_i < self.font_match_keys.len() {
            let id = self.font_match_keys[self.other_i].id;
            self.other_i += 1;
            if forbidden_families
                .iter()
                .all(|family_name| !self.face_contains_family(id, family_name))
            {
                if let Some(font) = self.font_system.get_font(id) {
                    return Some(font);
                }
            }
        }

        self.end = true;
        None
    }
}
