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

/// Verify that span-context shaping correctly distributes glyphs to word buckets.
///
/// When `should_shape_with_span_context` fires (ASCII, LTR, Advanced shaping, multi-word
/// span) the whole span is shaped first and its glyphs are split into per-word buckets.
/// This preserves cross-word OpenType contextual substitutions while still maintaining
/// the correct word structure for line-wrapping.
#[test]
fn test_span_context_shaping_glyph_distribution() {
    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};

    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/Inter-Regular.ttf").unwrap();
    font_system.db_mut().load_font_data(font);
    let metrics = Metrics::new(14.0, 20.0);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);

    // ASCII LTR multi-word → triggers the span-context path (should_shape_with_span_context=true).
    let text = "hello world";
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);

    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    let span = &shape.spans[0];

    // "hello world" → "hello" (non-blank), " " (blank), "world" (non-blank)
    assert_eq!(span.words.len(), 3, "Expected 3 words for 'hello world'");
    assert!(!span.words[0].blank, "'hello' word must be non-blank");
    assert!(span.words[1].blank, "space word must be blank");
    assert!(!span.words[2].blank, "'world' word must be non-blank");

    // Every word must have at least one glyph after the split.
    for (i, word) in span.words.iter().enumerate() {
        assert!(
            !word.glyphs.is_empty(),
            "word[{i}] has no glyphs (blank={})",
            word.blank
        );
    }

    // Glyph byte offsets must land in the correct region of the line.
    // "hello"=0..5, " "=5..6, "world"=6..11
    for glyph in &span.words[0].glyphs {
        assert!(
            glyph.start < 5,
            "'hello' glyph start {} is out of range 0..5",
            glyph.start
        );
    }
    for glyph in &span.words[1].glyphs {
        assert!(
            glyph.start == 5,
            "space glyph start {} is out of range 5..6",
            glyph.start
        );
    }
    for glyph in &span.words[2].glyphs {
        assert!(
            glyph.start >= 6,
            "'world' glyph start {} is out of range 6..11",
            glyph.start
        );
    }

    // Shaping::Basic falls through to per-word shaping but must still yield
    // the same word structure.
    buffer.set_text(text, &Attrs::new(), Shaping::Basic, None);
    buffer.shape_until_scroll(false);
    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    let span = &shape.spans[0];
    assert_eq!(span.words.len(), 3, "Basic shaping should also yield 3 words");
    assert!(!span.words[0].blank);
    assert!(span.words[1].blank);
    assert!(!span.words[2].blank);

    // A single word (no whitespace) must NOT trigger span-context shaping —
    // should_shape_with_span_context requires both whitespace and non-whitespace.
    buffer.set_text("hello", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);
    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    let span = &shape.spans[0];
    assert_eq!(span.words.len(), 1, "Single word should produce exactly 1 word");
    assert!(!span.words[0].blank);
    assert!(!span.words[0].glyphs.is_empty());
}

#[test]
fn test_ligature_segmentation() {
    use cosmic_text::{Buffer, FontSystem, Metrics, Shaping};

    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/Inter-Regular.ttf").unwrap();
    font_system.db_mut().load_font_data(font);
    let metrics = Metrics::new(14.0, 20.0);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);

    buffer.set_text("|>", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);

    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    let span = &shape.spans[0];

    // Inter-Regular HAS a contextual alternate for |> (changing the glyph ID),
    // so our probe detects it and keeps them together.
    assert_eq!(
        span.words.len(),
        1,
        "Expected '|>' to be 1 word (contextual alternate in Inter), but found {} words.",
        span.words.len()
    );

    // Test -> (Arrow), which is a common ligature.
    buffer.set_text("->", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);
    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");

    assert_eq!(
        shape.spans[0].words.len(),
        1,
        "Expected '->' to be a single word (ligature), but found {} words.",
        shape.spans[0].words.len()
    );

    // Test !=
    buffer.set_text("!=", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);
    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    // Inter has a contextual alternate for != too.
    assert_eq!(
        shape.spans[0].words.len(),
        1,
        "Expected '!=' to be 1 word (contextual alternate), but found {} words.",
        shape.spans[0].words.len()
    );

    // Test ++
    buffer.set_text("++", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);
    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    // Inter does not have a ++ ligature.
    assert_eq!(
        shape.spans[0].words.len(),
        2,
        "Expected '++' to be 2 words, but found {} words.",
        shape.spans[0].words.len()
    );
}
