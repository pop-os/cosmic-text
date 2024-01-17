// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::collections::BTreeSet;
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
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

// Match on lowest weight_offset, then script_non_matches
// Default font gets None for both `weight_offset` and `script_non_matches`, and thus, it is
// always the first to be popped from the set.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MonospaceFallbackInfo {
    weight_offset: Option<u16>,
    script_non_matches: Option<usize>,
    id: fontdb::ID,
}

pub struct FontFallbackIter<'a> {
    font_system: &'a mut FontSystem,
    font_match_keys: &'a [FontMatchKey],
    default_families: &'a [&'a Family<'a>],
    monospace_fallbacks: BTreeSet<MonospaceFallbackInfo>,
    default_i: usize,
    scripts: &'a [Script],
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
    ) -> Self {
        Self {
            font_system,
            font_match_keys,
            default_families,
            monospace_fallbacks: BTreeSet::new(),
            default_i: 0,
            scripts,
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
                .filter(move |m_key| m_key.weight_offset == 0 || is_mono)
        };

        while self.default_i < self.default_families.len() {
            self.default_i += 1;
            let is_mono = self.default_families[self.default_i - 1] == &Family::Monospace;

            for m_key in font_match_keys_iter(is_mono) {
                let default_family = self
                    .font_system
                    .db()
                    .family_name(self.default_families[self.default_i - 1]);
                if self.face_contains_family(m_key.id, default_family) {
                    if let Some(font) = self.font_system.get_font(m_key.id) {
                        if !is_mono {
                            return Some(font);
                        } else if m_key.weight_offset == 0 {
                            // Default font
                            let fallback_info = MonospaceFallbackInfo {
                                weight_offset: None,
                                script_non_matches: None,
                                id: m_key.id,
                            };
                            assert_eq!(self.monospace_fallbacks.insert(fallback_info), true);
                        }
                    }
                }
                // Set a monospace fallback if Monospace family is not found
                if is_mono {
                    let script_tags = self
                        .scripts
                        .iter()
                        .filter_map(|script| {
                            let script_as_lower = script.short_name().to_lowercase();
                            <[u8; 4]>::try_from(script_as_lower.as_bytes()).ok()
                        })
                        .collect::<Vec<_>>();

                    if let Some(face_info) = self.font_system.db().face(m_key.id) {
                        // Don't use emoji fonts as Monospace
                        if face_info.monospaced && !face_info.post_script_name.contains("Emoji") {
                            if let Some(font) = self.font_system.get_font(m_key.id) {
                                let script_non_matches = self.scripts.len()
                                    - script_tags
                                        .iter()
                                        .filter(|&&script_tag| {
                                            font.scripts()
                                                .iter()
                                                .any(|&tag_bytes| tag_bytes == script_tag)
                                        })
                                        .count();

                                let fallback_info = MonospaceFallbackInfo {
                                    weight_offset: Some(m_key.weight_offset),
                                    script_non_matches: Some(script_non_matches),
                                    id: m_key.id,
                                };
                                assert_eq!(self.monospace_fallbacks.insert(fallback_info), true);
                            }
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
