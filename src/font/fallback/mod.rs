// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use fontdb::Family;
use unicode_script::Script;

use crate::{Font, FontSystem};

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

pub struct FontFallbackIter<'a> {
    font_system: &'a mut FontSystem,
    font_ids: &'a [fontdb::ID],
    default_families: &'a [&'a Family<'a>],
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
        font_ids: &'a [fontdb::ID],
        default_families: &'a [&'a Family<'a>],
        scripts: &'a [Script],
    ) -> Self {
        Self {
            font_system,
            font_ids,
            default_families,
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
                self.face_name(self.font_ids[self.other_i - 1]),
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
        while self.default_i < self.default_families.len() {
            self.default_i += 1;
            let mut monospace_fallback = None;
            for id in self.font_ids.iter() {
                let default_family = self
                    .font_system
                    .db()
                    .family_name(self.default_families[self.default_i - 1]);
                if self.face_contains_family(*id, default_family) {
                    if let Some(font) = self.font_system.get_font(*id) {
                        return Some(font);
                    }
                }
                // Set a monospace fallback if Monospace family is not found
                if self.default_families[self.default_i - 1] == &Family::Monospace
                    && self.font_system.db().face(*id).map(|f| f.monospaced) == Some(true)
                    && monospace_fallback.is_none()
                {
                    monospace_fallback = Some(id);
                }
            }
            // If default family is Monospace fallback to first monospaced font
            if let Some(id) = monospace_fallback {
                if let Some(font) = self.font_system.get_font(*id) {
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
                for id in self.font_ids.iter() {
                    if self.face_contains_family(*id, script_family) {
                        if let Some(font) = self.font_system.get_font(*id) {
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
            for id in self.font_ids.iter() {
                if self.face_contains_family(*id, common_family) {
                    if let Some(font) = self.font_system.get_font(*id) {
                        return Some(font);
                    }
                }
            }
            log::debug!("failed to find family '{}'", common_family);
        }

        //TODO: do we need to do this?
        //TODO: do not evaluate fonts more than once!
        let forbidden_families = forbidden_fallback();
        while self.other_i < self.font_ids.len() {
            let id = self.font_ids[self.other_i];
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
