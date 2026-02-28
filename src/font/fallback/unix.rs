// SPDX-License-Identifier: MIT OR Apache-2.0

use super::Fallback;
use crate::fallback::platform_han_fallback;
use crate::HashMap;
use alloc::vec;
use unicode_script::Script;

/// A platform-specific font fallback list, for Windows.
pub fn platform_fallback(locale: &str) -> Fallback {
    let zh_hans = "Noto Sans CJK SC";
    let zh_hant_tw = "Noto Sans CJK TC";
    let zh_hant_hk = "Noto Sans CJK HK";
    let ja = "Noto Sans CJK JP";
    let ko = "Noto Sans CJK KR";
    let han = platform_han_fallback(locale, zh_hans, zh_hant_tw, zh_hant_hk, ja, ko);

    Fallback {
        common_fallback: vec![
            /* Sans-serif fallbacks */
            "Noto Sans".into(),
            /* More sans-serif fallbacks */
            "DejaVu Sans".into(),
            "FreeSans".into(),
            /* Mono fallbacks */
            "Noto Sans Mono".into(),
            "DejaVu Sans Mono".into(),
            "FreeMono".into(),
            /* Symbols fallbacks */
            "Noto Sans Symbols".into(),
            "Noto Sans Symbols2".into(),
            /* Emoji fallbacks*/
            "Noto Color Emoji".into(),
            //TODO: Add CJK script here for doublewides?
        ],
        forbidden_fallback: vec![],
        script_fallback: HashMap::from_iter([
            (
                Script::Adlam,
                vec!["Noto Sans Adlam".into(), "Noto Sans Adlam Unjoined".into()],
            ),
            (Script::Arabic, vec!["Noto Sans Arabic".into()]),
            (Script::Armenian, vec!["Noto Sans Armenian".into()]),
            (Script::Bengali, vec!["Noto Sans Bengali".into()]),
            (Script::Bopomofo, vec![han.into()]),
            //TODO: DejaVu Sans would typically be selected for braille characters,
            // but this breaks alignment when used alongside monospaced text.
            // By requesting the use of FreeMono first, this issue can be avoided.
            (Script::Braille, vec!["FreeMono".into()]),
            (Script::Buhid, vec!["Noto Sans Buhid".into()]),
            (Script::Chakma, vec!["Noto Sans Chakma".into()]),
            (Script::Cherokee, vec!["Noto Sans Cherokee".into()]),
            (Script::Deseret, vec!["Noto Sans Deseret".into()]),
            (Script::Devanagari, vec!["Noto Sans Devanagari".into()]),
            (Script::Ethiopic, vec!["Noto Sans Ethiopic".into()]),
            (Script::Georgian, vec!["Noto Sans Georgian".into()]),
            (Script::Gothic, vec!["Noto Sans Gothic".into()]),
            (Script::Grantha, vec!["Noto Sans Grantha".into()]),
            (Script::Gujarati, vec!["Noto Sans Gujarati".into()]),
            (Script::Gurmukhi, vec!["Noto Sans Gurmukhi".into()]),
            (Script::Han, vec![han.into()]),
            (Script::Hangul, vec![ko.into()]),
            (Script::Hanunoo, vec!["Noto Sans Hanunoo".into()]),
            (Script::Hebrew, vec!["Noto Sans Hebrew".into()]),
            (Script::Hiragana, vec![ja.into()]),
            (Script::Javanese, vec!["Noto Sans Javanese".into()]),
            (Script::Kannada, vec!["Noto Sans Kannada".into()]),
            (Script::Katakana, vec![ja.into()]),
            (Script::Khmer, vec!["Noto Sans Khmer".into()]),
            (Script::Lao, vec!["Noto Sans Lao".into()]),
            (Script::Malayalam, vec!["Noto Sans Malayalam".into()]),
            (Script::Mongolian, vec!["Noto Sans Mongolian".into()]),
            (Script::Myanmar, vec!["Noto Sans Myanmar".into()]),
            (Script::Oriya, vec!["Noto Sans Oriya".into()]),
            (Script::Runic, vec!["Noto Sans Runic".into()]),
            (Script::Sinhala, vec!["Noto Sans Sinhala".into()]),
            (Script::Syriac, vec!["Noto Sans Syriac".into()]),
            (Script::Tagalog, vec!["Noto Sans Tagalog".into()]),
            (Script::Tagbanwa, vec!["Noto Sans Tagbanwa".into()]),
            (Script::Tai_Le, vec!["Noto Sans Tai Le".into()]),
            (Script::Tai_Tham, vec!["Noto Sans Tai Tham".into()]),
            (Script::Tai_Viet, vec!["Noto Sans Tai Viet".into()]),
            (Script::Tamil, vec!["Noto Sans Tamil".into()]),
            (Script::Telugu, vec!["Noto Sans Telugu".into()]),
            (Script::Thaana, vec!["Noto Sans Thaana".into()]),
            (Script::Thai, vec!["Noto Sans Thai".into()]),
            //TODO: no sans script?
            (Script::Tibetan, vec!["Noto Serif Tibetan".into()]),
            (Script::Tifinagh, vec!["Noto Sans Tifinagh".into()]),
            (Script::Vai, vec!["Noto Sans Vai".into()]),
            //TODO: Use han_unification?
            (
                Script::Yi,
                vec!["Noto Sans Yi".into(), "Noto Sans CJK SC".into()],
            ),
        ]),
    }
}
