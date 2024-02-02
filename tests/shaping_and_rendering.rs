use common::DrawTestCfg;
use cosmic_text::Attrs;
use fontdb::Family;

mod common;

#[test]
fn test_hebrew_word_rendering() {
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("a_hebrew_word")
        .font_size(36., 40.)
        .font_attrs(attrs)
        .text("בדיקה")
        .canvas(120, 60)
        .validate_text_rendering();
}

#[test]
fn test_hebrew_paragraph_rendering() {
    let paragraph = "השועל החום המהיר קופץ מעל הכלב העצלן";
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("a_hebrew_paragraph")
        .font_size(36., 40.)
        .font_attrs(attrs)
        .text(paragraph)
        .canvas(400, 110)
        .validate_text_rendering();
}

#[test]
fn test_english_mixed_with_hebrew_paragraph_rendering() {
    let paragraph = "Many computer programs fail to display bidirectional text correctly. For example, this page is mostly LTR English script, and here is the RTL Hebrew name Sarah: שרה, spelled sin (ש) on the right, resh (ר) in the middle, and heh (ה) on the left.";
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("some_english_mixed_with_hebrew")
        .font_size(16., 20.)
        .font_attrs(attrs)
        .text(paragraph)
        .canvas(400, 120)
        .validate_text_rendering();
}

#[test]
fn test_arabic_word_rendering() {
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("an_arabic_word")
        .font_size(36., 40.)
        .font_attrs(attrs)
        .text("خالصة")
        .canvas(120, 60)
        .validate_text_rendering();
}

#[test]
fn test_arabic_paragraph_rendering() {
    let paragraph = "الثعلب البني السريع يقفز فوق الكلب الكسول";
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("an_arabic_paragraph")
        .font_size(36., 40.)
        .font_attrs(attrs)
        .text(paragraph)
        .canvas(400, 110)
        .validate_text_rendering();
}

#[test]
fn test_english_mixed_with_arabic_paragraph_rendering() {
    let paragraph = "I like to render اللغة العربية in Rust!";
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("some_english_mixed_with_arabic")
        .font_size(36., 40.)
        .font_attrs(attrs)
        .text(paragraph)
        .canvas(400, 110)
        .validate_text_rendering();
}
