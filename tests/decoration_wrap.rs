//! Decoration spans must follow their text across wrapped visual lines, and
//! must not cover glyphs between two separately decorated ranges.

use cosmic_text::{Attrs, AttrsList, Buffer, FontSystem, Metrics, Shaping, UnderlineStyle};

fn underline() -> Attrs<'static> {
    let mut attrs = Attrs::new();
    attrs.text_decoration.underline = UnderlineStyle::Single;
    attrs
}

/// Byte offsets of every glyph covered by a decoration span, per visual line.
fn decorated_offsets(buffer: &Buffer) -> Vec<Vec<usize>> {
    buffer
        .layout_runs()
        .map(|run| {
            let mut offsets: Vec<usize> = run
                .decorations
                .iter()
                .flat_map(|span| run.glyphs[span.glyph_range.clone()].iter())
                .map(|glyph| glyph.start)
                .collect();
            offsets.sort_unstable();
            offsets
        })
        .collect()
}

#[test]
fn wrapped_underline_covers_every_line_it_touches() {
    let mut fs = FontSystem::new();
    let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(120.0), None);

    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);

    // Underline a stretch that crosses several wrapped lines.
    let range = 6..47;
    let mut list = AttrsList::new(&Attrs::new());
    list.add_span(range.clone(), &underline());
    buffer.lines[0].set_attrs_list(list);
    buffer.shape_until_scroll(&mut fs, false);

    let line_count = buffer.layout_runs().count();
    assert!(
        line_count >= 3,
        "expected the text to wrap, got {line_count} lines"
    );

    for (line, offsets) in decorated_offsets(&buffer).iter().enumerate() {
        let expected: Vec<usize> = buffer
            .layout_runs()
            .nth(line)
            .unwrap()
            .glyphs
            .iter()
            .map(|glyph| glyph.start)
            .filter(|start| range.contains(start))
            .collect();
        let mut expected = expected;
        expected.sort_unstable();
        assert_eq!(
            *offsets, expected,
            "line {line}: decorated glyphs do not match the underlined range"
        );
    }
}

#[test]
fn separate_spans_do_not_bridge_the_gap_between_them() {
    let mut fs = FontSystem::new();
    let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(600.0), None);

    let text = "underlined plain underlined";
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);

    // Two identically styled spans separated by plain text.
    let mut list = AttrsList::new(&Attrs::new());
    list.add_span(0..10, &underline());
    list.add_span(17..27, &underline());
    buffer.lines[0].set_attrs_list(list);
    buffer.shape_until_scroll(&mut fs, false);

    let offsets = decorated_offsets(&buffer);
    let decorated = &offsets[0];
    assert!(!decorated.is_empty());
    assert!(
        decorated
            .iter()
            .all(|start| (0..10).contains(start) || (17..27).contains(start)),
        "decoration covers glyphs in the plain gap: {decorated:?}"
    );
}
