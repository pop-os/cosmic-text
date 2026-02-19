use common::DrawTestCfg;
use cosmic_text::{Align, Attrs, Ellipsize, EllipsizeHeightLimit, Family, Wrap};

mod common;

#[test]
fn test_ellipsize_ltr_end_single_line() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_ltr_end_single_line")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("The quick brown fox jumps over the lazy dog.")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
        .canvas(180, 50)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_ltr_end_single_line_aligned_right() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_ltr_end_single_line_aligned_right")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("The quick brown fox jumps over the lazy dog.")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
        .alignment(Some(Align::Right))
        .canvas(180, 50)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_rtl_end_single_line() {
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("ellipsize_rtl_end_single_line")
        .font_size(22., 28.)
        .font_attrs(attrs)
        .text("توانا بود هرکه دانا بود.")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
        .canvas(180, 55)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_mixed_end_single_line() {
    let attrs = Attrs::new().family(Family::Name("Noto Sans"));
    DrawTestCfg::new("ellipsize_mixed_end_single_line")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("Hello سلام mixed RTL/LTR world with extra words")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
        .canvas(190, 50)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_ltr_start_single_line() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_ltr_start_single_line")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("The quick brown fox jumps over the lazy dog.")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::Start(EllipsizeHeightLimit::Lines(1)))
        .canvas(180, 50)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_ltr_middle_single_line() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_ltr_middle_single_line")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("The quick brown fox jumps over the lazy dog.")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)))
        .canvas(180, 50)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_ltr_end_two_lines() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_ltr_end_two_lines")
        .font_size(18., 24.)
        .font_attrs(attrs)
        .text("Pack my box with five dozen liquor jugs. Sphinx of black quartz, judge my vow.")
        .wrap(Wrap::Word)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(2)))
        .canvas(200, 80)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_mixed_middle_single_line() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_mixed_middle_single_line")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("Hello سلام mixed RTL/LTR world with extra words")
        .wrap(Wrap::None)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)))
        .canvas(180, 50)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_mixed_ltr_rtl_middle_two_lines() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_mixed_ltr_rtl_middle_two_lines")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("First line is LTR خط دوم از راست به چپ")
        .wrap(Wrap::WordOrGlyph)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(2)))
        .canvas(180, 80)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_mixed_rtl_ltr_middle_two_lines() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_mixed_rtl_ltr_middle_two_lines")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("خط اول از راست به چپ Second line is LTR and has more words")
        .wrap(Wrap::WordOrGlyph)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(2)))
        .canvas(210, 80)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_ltr_single_word_middle_two_lines() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_ltr_single_word_middle_two_lines")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("AVeryLongWordThatExceedsTheWidth")
        .wrap(Wrap::WordOrGlyph)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(2)))
        .canvas(180, 80)
        .validate_text_rendering();
}

#[test]
fn test_ellipsize_mixed_ltr_rtl_ltr_middle_three_lines() {
    let attrs = Attrs::new().family(Family::Name("Inter"));
    DrawTestCfg::new("ellipsize_mixed_ltr_rtl_ltr_middle_three_lines")
        .font_size(20., 26.)
        .font_attrs(attrs)
        .text("This is some LTR text that keeps و یه مشت متن فارسیی.zippy")
        .wrap(Wrap::WordOrGlyph)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(3)))
        .canvas(200, 100)
        .validate_text_rendering();
}
