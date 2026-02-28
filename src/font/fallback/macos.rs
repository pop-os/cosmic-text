// SPDX-License-Identifier: MIT OR Apache-2.0

use super::Fallback;
use crate::fallback::platform_han_fallback;
use crate::HashMap;
use alloc::vec;
use unicode_script::Script;

/// A platform-specific font fallback list, for MacOS.
pub fn platform_fallback(locale: &str) -> Fallback {
    let zh_hans = "PingFang SC";
    let zh_hant_tw = "PingFang TC";
    let zh_hant_hk = "PingFang HK";
    let ja = "Hiragino Sans";
    let ko = "Apple SD Gothic Neo";
    let han = platform_han_fallback(locale, zh_hans, zh_hant_tw, zh_hant_hk, ja, ko);

    Fallback {
        common_fallback: vec![
            ".SF NS".into(),
            "Menlo".into(),
            "Apple Color Emoji".into(),
            "Geneva".into(),
            "Arial Unicode MS".into(),
        ],
        forbidden_fallback: vec![".LastResort".into()],
        //TODO: pull more data from about:config font.name-list.sans-serif in Firefox
        script_fallback: HashMap::from_iter([
            (Script::Adlam, vec!["Noto Sans Adlam".into()]),
            (Script::Arabic, vec!["Geeza Pro".into()]),
            (Script::Armenian, vec!["Noto Sans Armenian".into()]),
            (Script::Bengali, vec!["Bangla Sangam MN".into()]),
            (Script::Buhid, vec!["Noto Sans Buhid".into()]),
            (Script::Canadian_Aboriginal, vec!["Euphemia UCAS".into()]),
            (Script::Chakma, vec!["Noto Sans Chakma".into()]),
            (Script::Devanagari, vec!["Devanagari Sangam MN".into()]),
            (Script::Ethiopic, vec!["Kefa".into()]),
            (Script::Gothic, vec!["Noto Sans Gothic".into()]),
            (Script::Grantha, vec!["Grantha Sangam MN".into()]),
            (Script::Gujarati, vec!["Gujarati Sangam MN".into()]),
            (Script::Gurmukhi, vec!["Gurmukhi Sangam MN".into()]),
            (Script::Han, vec![han.into()]),
            (Script::Hangul, vec![ko.into()]),
            (Script::Hanunoo, vec!["Noto Sans Hanunoo".into()]),
            (Script::Hebrew, vec!["Arial".into()]),
            (Script::Hiragana, vec![ja.into()]),
            (Script::Javanese, vec!["Noto Sans Javanese".into()]),
            (Script::Kannada, vec!["Noto Sans Kannada".into()]),
            (Script::Katakana, vec![ja.into()]),
            (Script::Khmer, vec!["Khmer Sangam MN".into()]),
            (Script::Lao, vec!["Lao Sangam MN".into()]),
            (Script::Malayalam, vec!["Malayalam Sangam MN".into()]),
            (Script::Mongolian, vec!["Noto Sans Mongolian".into()]),
            (Script::Myanmar, vec!["Noto Sans Myanmar".into()]),
            (Script::Oriya, vec!["Noto Sans Oriya".into()]),
            (Script::Sinhala, vec!["Sinhala Sangam MN".into()]),
            (Script::Syriac, vec!["Noto Sans Syriac".into()]),
            (Script::Tagalog, vec!["Noto Sans Tagalog".into()]),
            (Script::Tagbanwa, vec!["Noto Sans Tagbanwa".into()]),
            (Script::Tai_Le, vec!["Noto Sans Tai Le".into()]),
            (Script::Tai_Tham, vec!["Noto Sans Tai Tham".into()]),
            (Script::Tai_Viet, vec!["Noto Sans Tai Viet".into()]),
            (Script::Tamil, vec!["InaiMathi".into()]),
            (Script::Telugu, vec!["Telugu Sangam MN".into()]),
            (Script::Thaana, vec!["Noto Sans Thaana".into()]),
            (Script::Thai, vec!["Ayuthaya".into()]),
            (Script::Tibetan, vec!["Kailasa".into()]),
            (Script::Tifinagh, vec!["Noto Sans Tifinagh".into()]),
            (Script::Vai, vec!["Noto Sans Vai".into()]),
            //TODO: Use han_unification?
            (
                Script::Yi,
                vec!["Noto Sans Yi".into(), "PingFang SC".into()],
            ),
        ]),
    }
}
