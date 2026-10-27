use cosmic_text::{Attrs, AttrsList, Buffer, FontSystem, Metrics, Shaping, UnderlineStyle};

fn underlined_buffer(
    font_system: &mut FontSystem,
    text: &str,
    span: std::ops::Range<usize>,
) -> Buffer {
    let mut buffer = Buffer::new(font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(600.0), None);
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);

    let mut underline = Attrs::new();
    underline.text_decoration.underline = UnderlineStyle::Single;
    let mut attrs_list = AttrsList::new(&Attrs::new());
    attrs_list.add_span(span, &underline);
    buffer.lines[0].set_attrs_list(attrs_list);
    buffer.shape_until_scroll(font_system, false);
    buffer
}

#[test]
fn x_range_covers_the_decorated_glyphs() {
    let mut font_system = FontSystem::new();
    let buffer = underlined_buffer(&mut font_system, "hello underline", 6..15);

    let run = buffer.layout_runs().next().unwrap();
    assert_eq!(run.decorations.len(), 1);
    let span = &run.decorations[0];
    let x_range = span.x_range(&run);

    let mut expected_min = f32::INFINITY;
    let mut expected_max = f32::NEG_INFINITY;
    for glyph in &run.glyphs[span.glyph_range.clone()] {
        expected_min = expected_min.min(glyph.x);
        expected_max = expected_max.max(glyph.x + glyph.w);
    }

    assert!(!x_range.is_empty());
    assert!(x_range.start > 0.0, "span starts after 'hello '");
    assert_eq!(x_range, expected_min..expected_max);
    assert!(x_range.end <= run.line_w);
}

#[test]
fn x_range_is_positive_for_rtl_text() {
    let mut font_system = FontSystem::new();
    let text = "שלום עולם";
    let buffer = underlined_buffer(&mut font_system, text, 0..text.len());

    let run = buffer.layout_runs().next().unwrap();
    assert!(run.rtl);
    assert_eq!(run.decorations.len(), 1);
    let x_range = run.decorations[0].x_range(&run);

    assert!(!x_range.is_empty());
    assert!(x_range.end > x_range.start);
}
