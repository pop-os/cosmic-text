use cosmic_text::{
    fontdb, Affinity, Attrs, Buffer, Cursor, Direction, FontSystem, Metrics, Shaping, Wrap,
};

fn font_system() -> FontSystem {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    font_system
        .db_mut()
        .load_font_data(std::fs::read("fonts/Inter-Regular.ttf").unwrap());
    font_system
        .db_mut()
        .load_font_data(std::fs::read("fonts/NotoSansArabic.ttf").unwrap());
    font_system
}

fn make_buffer(font_system: &mut FontSystem, text: &str, direction: Direction) -> Buffer {
    let mut buffer = Buffer::new(font_system, Metrics::new(14.0, 20.0));
    buffer.set_wrap(Wrap::None);
    buffer.set_direction(direction);
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    buffer
}

#[test]
fn affinity_picks_the_caret_side_at_a_direction_seam() {
    // "abc" (LTR) then Arabic (RTL): logical index 3 is the seam between two
    // runs of opposite direction, so the Before and After caret positions are
    // different x coordinates. Affinity must pick the side the cursor arrived
    // from: Before attaches to the glyph that ENDS at the index, After to the
    // glyph that STARTS there.
    let mut font_system = font_system();
    let buffer = make_buffer(&mut font_system, "abcسلام", Direction::Auto);
    let run = buffer
        .layout_runs()
        .next()
        .expect("expected at least one layout run");

    let seam = 3; // byte index: end of "abc", start of the Arabic run

    let (before_glyph, _) = run
        .cursor_glyph(&Cursor::new_with_affinity(0, seam, Affinity::Before))
        .expect("Before cursor at the seam must resolve");
    assert_eq!(
        run.glyphs[before_glyph].end, seam,
        "Affinity::Before must attach to the glyph that ends at the index"
    );

    let (after_glyph, _) = run
        .cursor_glyph(&Cursor::new_with_affinity(0, seam, Affinity::After))
        .expect("After cursor at the seam must resolve");
    assert_eq!(
        run.glyphs[after_glyph].start, seam,
        "Affinity::After must attach to the glyph that starts at the index"
    );

    // The two sides are distinct glyphs at a direction seam (in
    // single-direction text they would be the same x, and affinity is moot).
    assert_ne!(before_glyph, after_glyph);
}

#[test]
fn affinity_is_moot_inside_a_single_direction_run() {
    // In plain LTR text, end-of-glyph-N and start-of-glyph-N+1 are the same x;
    // both affinities must produce that same caret x.
    let mut font_system = font_system();
    let buffer = make_buffer(&mut font_system, "abcdef", Direction::Auto);
    let run = buffer
        .layout_runs()
        .next()
        .expect("expected at least one layout run");

    let index = 3;
    let x_of = |affinity| {
        let (glyph_i, offset) = run
            .cursor_glyph(&Cursor::new_with_affinity(0, index, affinity))
            .expect("cursor must resolve");
        run.glyphs[glyph_i].x + offset
    };
    let before_x = x_of(Affinity::Before);
    let after_x = x_of(Affinity::After);
    assert!(
        (before_x - after_x).abs() < 0.5,
        "affinity must not move the caret inside a single-direction run: before {before_x} vs after {after_x}"
    );
}
