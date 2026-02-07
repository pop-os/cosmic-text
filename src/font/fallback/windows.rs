// SPDX-License-Identifier: MIT OR Apache-2.0

use super::Fallback;
use crate::fallback::platform_han_fallback;
use crate::HashMap;
use alloc::vec;
use unicode_script::Script;

/// A platform-specific font fallback list, for Windows.
pub fn platform_fallback(locale: &str) -> Fallback {
    let zh_hans = "Microsoft YaHei UI";
    let zh_hant_tw = "Microsoft JhengHei UI";
    let zh_hant_hk = "MingLiU_HKSCS";
    let ja = "Yu Gothic";
    let ko = "Malgun Gothic";
    let han = platform_han_fallback(locale, zh_hans, zh_hant_tw, zh_hant_hk, ja, ko);

    Fallback {
        common_fallback: vec![
            "Segoe UI".into(),
            "Segoe UI Emoji".into(),
            "Segoe UI Symbol".into(),
            "Segoe UI Historic".into(),
            //TODO: Add CJK script here for doublewides?
        ],
        forbidden_fallback: vec![],
        script_fallback: HashMap::from_iter([
            (Script::Adlam, vec!["Ebrima".into()]),
            (Script::Bengali, vec!["Nirmala UI".into()]),
            (Script::Canadian_Aboriginal, vec!["Gadugi".into()]),
            (Script::Chakma, vec!["Nirmala UI".into()]),
            (Script::Cherokee, vec!["Gadugi".into()]),
            (Script::Devanagari, vec!["Nirmala UI".into()]),
            (Script::Ethiopic, vec!["Ebrima".into()]),
            (Script::Gujarati, vec!["Nirmala UI".into()]),
            (Script::Gurmukhi, vec!["Nirmala UI".into()]),
            (Script::Han, vec![han.into()]),
            (Script::Hangul, vec![ko.into()]),
            (Script::Hiragana, vec![ja.into()]),
            (Script::Javanese, vec!["Javanese Text".into()]),
            (Script::Kannada, vec!["Nirmala UI".into()]),
            (Script::Katakana, vec![ja.into()]),
            (Script::Khmer, vec!["Leelawadee UI".into()]),
            (Script::Lao, vec!["Leelawadee UI".into()]),
            (Script::Malayalam, vec!["Nirmala UI".into()]),
            (Script::Mongolian, vec!["Mongolian Baiti".into()]),
            (Script::Myanmar, vec!["Myanmar Text".into()]),
            (Script::Oriya, vec!["Nirmala UI".into()]),
            (Script::Sinhala, vec!["Nirmala UI".into()]),
            (Script::Tamil, vec!["Nirmala UI".into()]),
            (Script::Telugu, vec!["Nirmala UI".into()]),
            (Script::Thaana, vec!["MV Boli".into()]),
            (Script::Thai, vec!["Leelawadee UI".into()]),
            (Script::Tibetan, vec!["Microsoft Himalaya".into()]),
            (Script::Tifinagh, vec!["Ebrima".into()]),
            (Script::Vai, vec!["Ebrima".into()]),
            (Script::Yi, vec!["Microsoft Yi Baiti".into()]),
        ]),
    }
}
