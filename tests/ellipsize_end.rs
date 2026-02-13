use cosmic_text::{
    fontdb, Attrs, AttrsList, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics,
    ShapeWord, Shaping, Wrap,
};

#[test]
fn ellipsize_end_limits_lines_and_appends_ellipsis() {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/Inter-Regular.ttf").unwrap();
    font_system.db_mut().load_font_data(font);

    let attrs = Attrs::new().family(Family::Name("Inter"));
    let metrics = Metrics::new(14.0, 20.0);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_wrap(&mut font_system, Wrap::Word);
    buffer.set_ellipsize(
        &mut font_system,
        Ellipsize::End(EllipsizeHeightLimit::Lines(2)),
    );
    buffer.set_text(
        &mut font_system,
        "The quick brown fox jumps over the lazy dog. \
         The quick brown fox jumps over the lazy dog. \
         The quick brown fox jumps over the lazy dog.",
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.set_size(&mut font_system, Some(120.0), None);
    buffer.shape_until_scroll(&mut font_system, false);

    let runs: Vec<_> = buffer.layout_runs().collect();
    assert_eq!(runs.len(), 2, "expected ellipsize to limit lines to 2");

    let ellipsis_text = "\u{2026}";
    let ellipsis_word = ShapeWord::new(
        &mut font_system,
        ellipsis_text,
        &AttrsList::new(&attrs),
        0..ellipsis_text.len(),
        unicode_bidi::Level::ltr(),
        false,
        Shaping::Advanced,
    );
    let ellipsis_ids: Vec<u16> = ellipsis_word.glyphs.iter().map(|g| g.glyph_id).collect();
    assert!(
        !ellipsis_ids.is_empty(),
        "ellipsis glyphs should not be empty"
    );

    let last_run = runs.last().expect("missing last layout run");
    assert!(
        last_run.line_w <= 120.0 + f32::EPSILON,
        "ellipsized line should fit within width"
    );
    let mut tail_ids: Vec<u16> = last_run
        .glyphs
        .iter()
        .rev()
        .take(ellipsis_ids.len())
        .map(|g| g.glyph_id)
        .collect();
    tail_ids.reverse();
    assert_eq!(tail_ids, ellipsis_ids, "ellipsis should appear at end");
}
