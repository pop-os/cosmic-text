//! Decoration spans on RTL text must cover exactly the decorated glyphs,
//! with x extents that match those glyphs, on single and wrapped lines.

use cosmic_text::{Attrs, AttrsList, Buffer, FontSystem, Metrics, Shaping, UnderlineStyle};

fn underline() -> Attrs<'static> {
    let mut attrs = Attrs::new();
    attrs.text_decoration.underline = UnderlineStyle::Single;
    attrs
}

fn rtl_buffer(
    fs: &mut FontSystem,
    width: f32,
    text: &str,
    range: std::ops::Range<usize>,
) -> Buffer {
    let mut buffer = Buffer::new(fs, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(width), None);
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);

    let mut list = AttrsList::new(&Attrs::new());
    list.add_span(range, &underline());
    buffer.lines[0].set_attrs_list(list);
    buffer.shape_until_scroll(fs, false);
    buffer
}

/// Per line: (byte offsets covered by decorations, decoration x extents,
/// expected byte offsets, expected x extents from the raw glyphs).
fn check_lines(buffer: &Buffer, range: &std::ops::Range<usize>) {
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

        for span in run.decorations {
            let x_range = span.x_range(&run);
            let mut x_min = f32::INFINITY;
            let mut x_max = f32::NEG_INFINITY;
            for glyph in &run.glyphs[span.glyph_range.clone()] {
                assert!(
                    range.contains(&glyph.start),
                    "line {line}: decoration covers glyph at byte {} outside {range:?}",
                    glyph.start
                );
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
}

#[test]
fn rtl_single_line_underline_extent_matches_its_glyphs() {
    // Hebrew, Article 3 of the UDHR, with the rights phrase underlined.
    let prefix = "כל אדם יש לו הזכות ";
    let phrase = "לחיים, לחרות ולבטחון אישי";
    let text = format!("{prefix}{phrase}.");
    let range = prefix.len()..prefix.len() + phrase.len();

    let mut fs = FontSystem::new();
    let buffer = rtl_buffer(&mut fs, 600.0, &text, range.clone());
    assert_eq!(buffer.layout_runs().count(), 1);
    assert!(buffer.layout_runs().next().unwrap().rtl);

    check_lines(&buffer, &range);
}

#[test]
fn rtl_aligned_metadata_spans_match_their_glyphs() {
    use cosmic_text::{Align, Color, Weight};

    // Mirror iced's rich_text: one attrs span per rich span, tagged with
    // metadata, the underlined one colored, plus a bold span, right-aligned.
    let a = "כל בני אדם נולדו ";
    let b = "בני חורין ושווים בערכם ובזכויותיהם";
    let c = ". כולם חוננו ";
    let d = "בתבונה ובמצפון";
    let e = ", לפיכך חובה עליהם לנהוג איש ברעהו ברוח של אחוה.";
    let text = format!("{a}{b}{c}{d}{e}");
    let range = a.len()..a.len() + b.len();

    let mut fs = FontSystem::new();
    let mut buffer = Buffer::new(&mut fs, Metrics::new(16.0, 26.0));
    buffer.set_size(Some(880.0), None);
    buffer.set_text(&text, &Attrs::new(), Shaping::Advanced, None);

    let mut list = AttrsList::new(&Attrs::new());
    let mut plain = Attrs::new();
    plain.metadata = 0;
    list.add_span(0..a.len(), &plain);
    let mut underlined = underline();
    underlined.metadata = 1;
    underlined.color_opt = Some(Color::rgb(0x40, 0xC0, 0x40));
    list.add_span(range.clone(), &underlined);
    let mut mid = Attrs::new();
    mid.metadata = 2;
    list.add_span(range.end..range.end + c.len(), &mid);
    let mut bold = Attrs::new();
    bold.metadata = 3;
    bold.weight = Weight::BOLD;
    list.add_span(range.end + c.len()..range.end + c.len() + d.len(), &bold);
    let mut tail = Attrs::new();
    tail.metadata = 4;
    list.add_span(range.end + c.len() + d.len()..text.len(), &tail);

    buffer.lines[0].set_attrs_list(list);
    buffer.lines[0].set_align(Some(Align::Right));
    buffer.shape_until_scroll(&mut fs, false);

    check_lines(&buffer, &range);

    // Every decoration span must start on a glyph of the underlined rich
    // span, so a consumer keying on first-glyph metadata finds it.
    for run in buffer.layout_runs() {
        for span in run.decorations {
            assert_eq!(run.glyphs[span.glyph_range.start].metadata, 1);
        }
    }
}

#[test]
fn rtl_wrapped_underline_covers_every_line_it_touches() {
    // Hebrew, Article 1 of the UDHR, underline crossing wrapped lines.
    let prefix = "כל בני אדם נולדו ";
    let phrase = "בני חורין ושווים בערכם ובזכויותיהם";
    let text = format!(
        "{prefix}{phrase}. כולם חוננו בתבונה ובמצפון, לפיכך חובה עליהם לנהוג איש ברעהו ברוח של אחוה."
    );
    let range = prefix.len()..prefix.len() + phrase.len();

    let mut fs = FontSystem::new();
    let buffer = rtl_buffer(&mut fs, 160.0, &text, range.clone());
    let lines = buffer.layout_runs().count();
    assert!(lines >= 3, "expected the text to wrap, got {lines} lines");

    check_lines(&buffer, &range);
}
