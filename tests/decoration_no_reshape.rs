//! A decoration-only change must not reshape the line, while a
//! shaping-relevant change (font size) still does.
//!
//! `BufferLine::set_attrs_list` returns `true` iff it reset shaping; these
//! tests assert on that signal and confirm the shaped glyph identities are
//! unchanged across a decoration toggle, and that the decoration still renders.

use cosmic_text::{
    Attrs, AttrsList, Buffer, CacheMetrics, FontSystem, Metrics, Shaping, UnderlineStyle,
};

fn glyph_ids(buffer: &Buffer) -> Vec<(cosmic_text::fontdb::ID, u16, f32)> {
    buffer
        .layout_runs()
        .flat_map(|run| {
            run.glyphs
                .iter()
                .map(|glyph| (glyph.font_id, glyph.glyph_id, glyph.w))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn shaped_buffer(fs: &mut FontSystem) -> Buffer {
    let mut buffer = Buffer::new(fs, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(400.0), None);
    buffer.set_text(
        "hello underlined world",
        &Attrs::new(),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(fs, false);
    buffer
}

#[test]
fn decoration_only_change_does_not_reshape() {
    let mut fs = FontSystem::new();
    let mut buffer = shaped_buffer(&mut fs);
    let before = glyph_ids(&buffer);
    assert!(!before.is_empty());

    // Underline a span: a decoration-only change.
    let mut under = Attrs::new();
    under.text_decoration.underline = UnderlineStyle::Single;
    let mut list = AttrsList::new(&Attrs::new());
    list.add_span(6..16, &under);

    let reshaped = buffer.lines[0].set_attrs_list(list);
    assert!(!reshaped, "a decoration-only change must not reshape");

    buffer.shape_until_scroll(&mut fs, false);
    assert_eq!(
        before,
        glyph_ids(&buffer),
        "shaped glyphs changed on a decoration-only change"
    );

    // ...and the decoration is resolved over those glyphs at layout.
    let has_decoration = buffer.layout_runs().any(|run| !run.decorations.is_empty());
    assert!(has_decoration, "decoration was not applied");
}

#[test]
fn font_size_change_reshapes() {
    let mut fs = FontSystem::new();
    let mut buffer = shaped_buffer(&mut fs);

    let mut big = Attrs::new();
    big.metrics_opt = Some(CacheMetrics::from(Metrics::new(24.0, 30.0)));
    let mut list = AttrsList::new(&Attrs::new());
    list.add_span(0..5, &big);

    let reshaped = buffer.lines[0].set_attrs_list(list);
    assert!(reshaped, "a font-size change must reshape (control)");
}
