//! Decoration spans must cover exactly the decorated glyphs when the buffer
//! has an explicit base direction, for LTR and RTL text, single and wrapped.

use cosmic_text::{
    Attrs, AttrsList, Buffer, Direction, FontSystem, Metrics, Shaping, UnderlineStyle,
};
use std::ops::Range;

fn underline() -> Attrs<'static> {
    let mut attrs = Attrs::new();
    attrs.text_decoration.underline = UnderlineStyle::Single;
    attrs
}

fn underlined_buffer(
    fs: &mut FontSystem,
    width: f32,
    direction: Direction,
    text: &str,
    range: Range<usize>,
) -> Buffer {
    let mut buffer = Buffer::new(fs, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(width), None);
    buffer.set_direction(direction);
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);

    let mut list = AttrsList::new(&Attrs::new());
    list.add_span(range, &underline());
    buffer.lines[0].set_attrs_list(list);
    buffer.shape_until_scroll(fs, false);
    buffer
}

fn first_run_rtl(buffer: &Buffer) -> bool {
    buffer
        .layout_runs()
        .next()
        .expect("expected at least one layout run")
        .rtl
}

/// Per line: the decorated glyphs must be exactly those inside `range`, and
/// each decoration's x extent must match its glyphs. At least one glyph must
/// be decorated overall, so the check cannot pass vacuously.
fn check_lines(buffer: &Buffer, range: &Range<usize>) {
    let mut decorated = 0;
    for (line, run) in buffer.layout_runs().enumerate() {
        let mut covered: Vec<usize> = run
            .decorations
            .iter()
            .flat_map(|span| run.glyphs[span.glyph_range.clone()].iter())
            .map(|glyph| glyph.start)
            .collect();
        covered.sort_unstable();

        let mut expected: Vec<usize> = run
            .glyphs
            .iter()
            .map(|glyph| glyph.start)
            .filter(|start| range.contains(start))
            .collect();
        expected.sort_unstable();

        assert_eq!(
            covered, expected,
            "line {line}: decorated glyphs do not match the underlined range"
        );
        decorated += covered.len();

        for span in run.decorations {
            let x_range = span.x_range(&run);
            let mut x_min = f32::INFINITY;
            let mut x_max = f32::NEG_INFINITY;
            for glyph in &run.glyphs[span.glyph_range.clone()] {
                x_min = x_min.min(glyph.x);
                x_max = x_max.max(glyph.x + glyph.w);
            }
            assert_eq!(
                x_range,
                x_min..x_max,
                "line {line}: decoration x extent does not match its glyphs"
            );
        }
    }
    assert!(decorated > 0, "no glyph was decorated");
}

/// Hebrew, Article 1 of the UDHR, with the rights phrase underlined.
fn hebrew() -> (String, Range<usize>) {
    let prefix = "כל בני אדם נולדו ";
    let phrase = "בני חורין ושווים בערכם ובזכויותיהם";
    let text = format!(
        "{prefix}{phrase}. כולם חוננו בתבונה ובמצפון, לפיכך חובה עליהם לנהוג איש ברעהו ברוח של אחוה."
    );
    (text, prefix.len()..prefix.len() + phrase.len())
}

/// English, Article 1 of the UDHR, with the rights phrase underlined.
fn latin() -> (String, Range<usize>) {
    let prefix = "All human beings are born ";
    let phrase = "free and equal in dignity and rights";
    let text = format!(
        "{prefix}{phrase}. They are endowed with reason and conscience and should act towards one another in a spirit of brotherhood."
    );
    (text, prefix.len()..prefix.len() + phrase.len())
}

/// Latin then Hebrew, underlined from mid-Latin through the first Hebrew
/// word, so the underline crosses the script boundary.
fn mixed() -> (String, Range<usize>) {
    let prefix = "hello there ";
    let latin = "brave new ";
    let hebrew_word = "שלום";
    let text = format!("{prefix}{latin}{hebrew_word} עולם goodbye");
    let start = prefix.len();
    (text, start..start + latin.len() + hebrew_word.len())
}

#[test]
fn forced_rtl_base_keeps_hebrew_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = hebrew();
    let buffer = underlined_buffer(
        &mut fs,
        2000.0,
        Direction::RightToLeft,
        &text,
        range.clone(),
    );
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_rtl_base_keeps_wrapped_hebrew_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = hebrew();
    let buffer = underlined_buffer(&mut fs, 160.0, Direction::RightToLeft, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 3, "expected the text to wrap, got {lines} lines");
    assert!(first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_ltr_base_keeps_hebrew_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = hebrew();
    let buffer = underlined_buffer(
        &mut fs,
        2000.0,
        Direction::LeftToRight,
        &text,
        range.clone(),
    );
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(!first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_ltr_base_keeps_wrapped_hebrew_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = hebrew();
    let buffer = underlined_buffer(&mut fs, 160.0, Direction::LeftToRight, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 3, "expected the text to wrap, got {lines} lines");
    assert!(!first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_rtl_base_keeps_latin_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = latin();
    let buffer = underlined_buffer(
        &mut fs,
        2000.0,
        Direction::RightToLeft,
        &text,
        range.clone(),
    );
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_rtl_base_keeps_wrapped_latin_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = latin();
    let buffer = underlined_buffer(&mut fs, 160.0, Direction::RightToLeft, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 3, "expected the text to wrap, got {lines} lines");
    assert!(first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_ltr_base_keeps_latin_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = latin();
    let buffer = underlined_buffer(
        &mut fs,
        2000.0,
        Direction::LeftToRight,
        &text,
        range.clone(),
    );
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(!first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_ltr_base_keeps_wrapped_latin_underline_on_its_glyphs() {
    let mut fs = FontSystem::new();
    let (text, range) = latin();
    let buffer = underlined_buffer(&mut fs, 160.0, Direction::LeftToRight, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 3, "expected the text to wrap, got {lines} lines");
    assert!(!first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_ltr_base_keeps_underline_across_script_boundary() {
    let mut fs = FontSystem::new();
    let (text, range) = mixed();
    let buffer = underlined_buffer(
        &mut fs,
        2000.0,
        Direction::LeftToRight,
        &text,
        range.clone(),
    );
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(!first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_ltr_base_keeps_wrapped_underline_across_script_boundary() {
    let mut fs = FontSystem::new();
    let (text, range) = mixed();
    let buffer = underlined_buffer(&mut fs, 90.0, Direction::LeftToRight, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 2, "expected the text to wrap, got {lines} lines");
    assert!(!first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_rtl_base_keeps_underline_across_script_boundary() {
    let mut fs = FontSystem::new();
    let (text, range) = mixed();
    let buffer = underlined_buffer(
        &mut fs,
        2000.0,
        Direction::RightToLeft,
        &text,
        range.clone(),
    );
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}

#[test]
fn forced_rtl_base_keeps_wrapped_underline_across_script_boundary() {
    let mut fs = FontSystem::new();
    let (text, range) = mixed();
    let buffer = underlined_buffer(&mut fs, 90.0, Direction::RightToLeft, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 2, "expected the text to wrap, got {lines} lines");
    assert!(first_run_rtl(&buffer));

    check_lines(&buffer, &range);
}
