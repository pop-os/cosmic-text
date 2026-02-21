// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem;
use fontdb::Family;
use unicode_script::Script;

use crate::{Font, FontMatchKey, FontSystem, HashMap, ShapeBuffer};

#[cfg(not(any(all(unix, not(target_os = "android")), target_os = "windows")))]
#[path = "other.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;

#[cfg(all(unix, not(any(target_os = "android", target_os = "macos"))))]
#[path = "unix.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

macro_rules! enum_fallback_entry {
    ($doc:expr, $ident:ident { $($variant:ident($ty:ty),)+ }) => {
        #[derive(Clone, Debug)]
        #[doc = $doc]
        pub enum $ident {
            $($variant($ty),)+
        }

        $(
            impl From<$ty> for $ident {
                fn from(value: $ty) -> Self {
                    Self::$variant(value)
                }
            }
        )+

        impl AsRef<str> for $ident {
            fn as_ref(&self) -> &str {
                match self {
                    $(Self::$variant(x) => x.as_ref(),)+
                }
            }
        }
    };
}

enum_fallback_entry!(
    "See [Fallback].",
    FallbackEntry {
        Owned(String),
        Static(&'static str),
        ArcStr(Arc<str>),
        ArcString(Arc<String>),
    }
);

//TODO: abstract style (sans/serif/monospaced)
/// The `Fallback` struct allows for configurable font fallback lists to be set for the [`FontSystem`].
///
/// A custom fallback list can be added via the [`FontSystem::new_with_locale_and_db_and_fallback`] constructor, and can be modified by modifying [`FontSystem::fallback_mut`].
///
/// A default fallback list is provided by the [`platform_fallback`] function, which returns a pre-configured fallback list for the target platform.
///
/// Example:
/// ```rust
/// use unicode_script::Script;
/// use cosmic_text::{Fallback, FontSystem};
/// use std::collections::HashMap;
///
/// let fallback = Fallback {
///     common_fallback: vec![
///         "Common Font A".into(),
///         "Common Font B".into(),
///     ],
///     forbidden_fallback: vec![],
///     script_fallback: HashMap::from_iter([
///         (Script::Adlam, vec!["Adlam Font".into()]),
///     ]),
/// };
/// let locale = "en-US".to_string();
/// let db = fontdb::Database::new();
/// let font_system = FontSystem::new_with_locale_and_db_and_fallback(locale, db, fallback);
/// ```
#[derive(Clone, Debug)]
pub struct Fallback {
    /// Fallbacks to use after any script specific fallbacks
    pub common_fallback: Vec<FallbackEntry>,
    /// Fallbacks to never use
    pub forbidden_fallback: Vec<FallbackEntry>,
    /// Fallbacks to use per script
    pub script_fallback: HashMap<Script, Vec<FallbackEntry>>,
}

pub use platform::platform_fallback;

#[cfg(not(feature = "warn_on_missing_glyphs"))]
use log::debug as missing_warn;
#[cfg(feature = "warn_on_missing_glyphs")]
use log::warn as missing_warn;

// Match on lowest font_weight_diff, then script_non_matches, then font_weight
// Default font gets None for both `weight_offset` and `script_non_matches`, and thus, it is
// always the first to be popped from the set.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonospaceFallbackInfo {
    font_weight_diff: Option<u16>,
    codepoint_non_matches: Option<usize>,
    font_weight: u16,
    id: fontdb::ID,
}

#[derive(Debug)]
pub struct FontFallbackIter<'a> {
    font_system: &'a mut FontSystem,
    font_match_keys: &'a [FontMatchKey],
    default_families: &'a [&'a Family<'a>],
    default_i: usize,
    scripts: &'a [Script],
    word: &'a str,
    script_i: (usize, usize),
    common_i: usize,
    other_i: usize,
    end: bool,
    ideal_weight: fontdb::Weight,
}

impl<'a> FontFallbackIter<'a> {
    pub fn new(
        font_system: &'a mut FontSystem,
        font_match_keys: &'a [FontMatchKey],
        default_families: &'a [&'a Family<'a>],
        scripts: &'a [Script],
        word: &'a str,
        ideal_weight: fontdb::Weight,
    ) -> Self {
        font_system.monospace_fallbacks_buffer.clear();
        Self {
            font_system,
            font_match_keys,
            default_families,
            default_i: 0,
            scripts,
            word,
            script_i: (0, 0),
            common_i: 0,
            other_i: 0,
            end: false,
            ideal_weight,
        }
    }

    pub fn check_missing(&self, word: &str) {
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
            let family = &self.font_system.fallback().common_fallback[self.common_i - 1];
            missing_warn!(
                "Failed to find script fallback for {:?} locale '{}', used '{:?}': '{}'",
                self.scripts,
                self.font_system.locale(),
                family,
                word
            );
        }
    }

    pub fn face_name(&self, id: fontdb::ID) -> &str {
        self.font_system
            .db()
            .face(id)
            .map_or("invalid font id", |face| {
                if let Some((name, _)) = face.families.first() {
                    name
                } else {
                    &face.post_script_name
                }
            })
    }

    pub fn shape_caches(&mut self) -> &mut ShapeBuffer {
        &mut self.font_system.shape_buffer
    }

    fn face_contains_family(&self, id: fontdb::ID, family_name: &str) -> bool {
        self.font_system
            .db()
            .face(id)
            .is_some_and(|face| face.families.iter().any(|(name, _)| name == family_name))
    }

    fn default_font_match_key(&self) -> Option<&FontMatchKey> {
        let default_family = self.default_families[self.default_i - 1];
        let default_family_name = self.font_system.db().family_name(default_family);

        self.font_match_keys
            .iter()
            .filter(|m_key| m_key.font_weight_diff == 0)
            .find(|m_key| self.face_contains_family(m_key.id, default_family_name))
    }

    fn next_item(&mut self, fallback: &Fallback) -> Option<<Self as Iterator>::Item> {
        if let Some(fallback_info) = self.font_system.monospace_fallbacks_buffer.pop_first() {
            if let Some(font) = self
                .font_system
                .get_font(fallback_info.id, self.ideal_weight)
            {
                return Some(font);
            }
        }

        'DEF_FAM: while self.default_i < self.default_families.len() {
            self.default_i += 1;
            let is_mono = self.default_families[self.default_i - 1] == &Family::Monospace;
            let default_font_match_key = self.default_font_match_key().copied();
            let word_chars_count = self.word.chars().count();

            macro_rules! mk_mono_fallback_info {
                ($m_key:expr) => {{
                    let supported_cp_count_opt =
                        self.font_system.get_font_supported_codepoints_in_word(
                            $m_key.id,
                            self.ideal_weight,
                            self.word,
                        );

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
                    if let Some(font) = self.font_system.get_font(m_key.id, self.ideal_weight) {
                        return Some(font);
                    }
                    break 'DEF_FAM;
                }
                (true, None) => (),
                (true, Some(m_key)) => {
                    // Default Monospace font
                    if let Some(mut fallback_info) = mk_mono_fallback_info!(m_key) {
                        fallback_info.font_weight_diff = None;

                        // Return early if default Monospace font supports all word codepoints.
                        // Otherewise, add to fallbacks set
                        if fallback_info.codepoint_non_matches == Some(0) {
                            if let Some(font) =
                                self.font_system.get_font(m_key.id, self.ideal_weight)
                            {
                                return Some(font);
                            }
                        } else {
                            assert!(self
                                .font_system
                                .monospace_fallbacks_buffer
                                .insert(fallback_info));
                        }
                    }
                }
            }

            let mono_ids_for_scripts = if is_mono && !self.scripts.is_empty() {
                let scripts = self.scripts.iter().filter_map(|script| {
                    let script_as_lower = script.short_name().to_lowercase();
                    <[u8; 4]>::try_from(script_as_lower.as_bytes()).ok()
                });
                self.font_system.get_monospace_ids_for_scripts(scripts)
            } else {
                Vec::new()
            };

            for m_key in self.font_match_keys.iter() {
                if Some(m_key.id) != default_font_match_key.as_ref().map(|m_key| m_key.id) {
                    let is_mono_id = if mono_ids_for_scripts.is_empty() {
                        self.font_system.is_monospace(m_key.id)
                    } else {
                        mono_ids_for_scripts.binary_search(&m_key.id).is_ok()
                    };

                    if is_mono_id {
                        let supported_cp_count_opt =
                            self.font_system.get_font_supported_codepoints_in_word(
                                m_key.id,
                                self.ideal_weight,
                                self.word,
                            );
                        if let Some(supported_cp_count) = supported_cp_count_opt {
                            let codepoint_non_matches =
                                self.word.chars().count() - supported_cp_count;

                            let fallback_info = MonospaceFallbackInfo {
                                font_weight_diff: Some(m_key.font_weight_diff),
                                codepoint_non_matches: Some(codepoint_non_matches),
                                font_weight: m_key.font_weight,
                                id: m_key.id,
                            };
                            assert!(self
                                .font_system
                                .monospace_fallbacks_buffer
                                .insert(fallback_info));
                        }
                    }
                }
            }
            // If default family is Monospace fallback to first monospaced font
            if let Some(fallback_info) = self.font_system.monospace_fallbacks_buffer.pop_first() {
                if let Some(font) = self
                    .font_system
                    .get_font(fallback_info.id, self.ideal_weight)
                {
                    return Some(font);
                }
            }
        }

        while self.script_i.0 < self.scripts.len() {
            let script = self.scripts[self.script_i.0];

            let script_families = fallback
                .script_fallback
                .get(&script)
                .map(|x| x.as_slice())
                .unwrap_or_default();

            while self.script_i.1 < script_families.len() {
                let script_family = &script_families[self.script_i.1];
                self.script_i.1 += 1;
                for m_key in self.font_match_keys.iter() {
                    if self.face_contains_family(m_key.id, script_family.as_ref()) {
                        if let Some(font) = self.font_system.get_font(m_key.id, self.ideal_weight) {
                            return Some(font);
                        }
                    }
                }
                log::debug!(
                    "failed to find family '{:?}' for script {:?} and locale '{}'",
                    script_family,
                    script,
                    self.font_system.locale(),
                );
            }

            self.script_i.0 += 1;
            self.script_i.1 = 0;
        }

        let common_families = &fallback.common_fallback;
        while self.common_i < common_families.len() {
            let common_family = &common_families[self.common_i];
            self.common_i += 1;
            for m_key in self.font_match_keys.iter() {
                if self.face_contains_family(m_key.id, common_family.as_ref()) {
                    if let Some(font) = self.font_system.get_font(m_key.id, self.ideal_weight) {
                        return Some(font);
                    }
                }
            }
            log::debug!("failed to find family '{common_family:?}'");
        }

        //TODO: do we need to do this?
        //TODO: do not evaluate fonts more than once!
        let forbidden_families = &fallback.forbidden_fallback;
        while self.other_i < self.font_match_keys.len() {
            let id = self.font_match_keys[self.other_i].id;
            self.other_i += 1;
            if forbidden_families
                .iter()
                .all(|family_name| !self.face_contains_family(id, family_name.as_ref()))
            {
                if let Some(font) = self.font_system.get_font(id, self.ideal_weight) {
                    return Some(font);
                }
            }
        }

        self.end = true;
        None
    }
}

impl Iterator for FontFallbackIter<'_> {
    type Item = Arc<Font>;
    fn next(&mut self) -> Option<Self::Item> {
        // The `mem::swap`s are a workaround for double borrowing (E0502).
        let mut fallback = Fallback {
            common_fallback: Default::default(),
            forbidden_fallback: Default::default(),
            script_fallback: Default::default(),
        };
        mem::swap(&mut fallback, self.font_system.fallback_mut());
        let item = self.next_item(&fallback);
        mem::swap(&mut fallback, self.font_system.fallback_mut());
        item
    }
}

fn platform_han_fallback(
    locale: &str,
    zh_hans: &'static str,
    zh_hant_tw: &'static str,
    zh_hant_hk: &'static str,
    ja: &'static str,
    ko: &'static str,
) -> &'static str {
    // TODO: Use a proper locale parsing library
    let subtags = locale
        .split('-')
        .map(|x| x.to_ascii_lowercase())
        .collect::<Vec<_>>();

    match subtags.first().map(|x| x.as_str()).unwrap_or_default() {
        "ja" => ja,
        "ko" => ko,
        _ => {
            if subtags.len() <= 1 || subtags.iter().skip(1).any(|x| x == "hans") {
                zh_hans
            } else if subtags.iter().skip(1).any(|x| x == "hk") {
                zh_hant_hk
            } else if subtags.iter().skip(1).any(|x| x == "hant" || x == "tw") {
                zh_hant_tw
            } else {
                zh_hans
            }
        }
    }
}

#[test]
fn test_platform_han_fallback() {
    let zh_hans = "zh_hans";
    let zh_hant_tw = "zh_hant_tw";
    let zh_hant_hk = "zh_hant_hk";
    let ja = "ja";
    let ko = "ko";
    let han = |locale: &str| platform_han_fallback(locale, zh_hans, zh_hant_tw, zh_hant_hk, ja, ko);

    assert_eq!(han(""), zh_hans);
    assert_eq!(han("en"), zh_hans);
    assert_eq!(han("-"), zh_hans);

    assert_eq!(han("ja"), ja);
    assert_eq!(han("ko"), ko);

    assert_eq!(han("zh"), zh_hans);

    assert_eq!(han("zh-Hans"), zh_hans);
    assert_eq!(han("zh-Hans-CN"), zh_hans);
    assert_eq!(han("zh-Hans-HK"), zh_hans);
    assert_eq!(han("zh-Hans-TW"), zh_hans);
    assert_eq!(han("zh-Hans-SG"), zh_hans);

    assert_eq!(han("zh-Hant"), zh_hant_tw);
    assert_eq!(han("zh-Hant-CN"), zh_hant_tw);
    assert_eq!(han("zh-Hant-HK"), zh_hant_hk);
    assert_eq!(han("zh-Hant-TW"), zh_hant_tw);
    assert_eq!(han("zh-Hant-SG"), zh_hant_tw);

    assert_eq!(han("zh-CN"), zh_hans);
    assert_eq!(han("zh-HK"), zh_hant_hk);
    assert_eq!(han("zh-TW"), zh_hant_tw);
    assert_eq!(han("zh-SG"), zh_hans);
}
