use cosmic_text::{
    fontdb, Attrs, AttrsList, Buffer, Ellipsize, Family, FontSystem, Metrics, ShapeWord, Shaping,
    Wrap,
};

#[test]
fn ellipsize_start_inserts_ellipsis_glyphs() {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/Inter-Regular.ttf").unwrap();
    font_system.db_mut().load_font_data(font);

    let attrs = Attrs::new().family(Family::Name("Inter"));
    let metrics = Metrics::new(14.0, 20.0);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_wrap(&mut font_system, Wrap::None);
    buffer.set_ellipsize(&mut font_system, Ellipsize::Start);
    buffer.set_text(
        &mut font_system,
        "abcdefghijklmnopqrstuvwxyz",
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.set_size(&mut font_system, Some(40.0), None);
    buffer.shape_until_scroll(&mut font_system, false);

    let run = buffer.layout_runs().next().expect("missing layout run");

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
    assert!(!ellipsis_ids.is_empty());

    assert!(run.line_w <= 40.0 + f32::EPSILON);
    let run_ids: Vec<u16> = run
        .glyphs
        .iter()
        .take(ellipsis_ids.len())
        .map(|g| g.glyph_id)
        .collect();
    assert_eq!(run_ids, ellipsis_ids);
}
