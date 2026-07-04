use cosmic_text::{fontdb, Align, Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};

fn font_system() -> FontSystem {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    font_system
        .db_mut()
        .load_font_data(std::fs::read("fonts/Inter-Regular.ttf").unwrap());
    font_system
}

#[test]
fn click_before_a_right_aligned_line_hits_its_start() {
    // A short right-aligned line starts at glyphs[0].x > 0. A click in the
    // leading gap (left of the text, but x >= 0) is "before the line" and must
    // place the cursor at the line START. The old test was hardcoded `x < 0.0`,
    // so such clicks fell through to the past-all-glyphs arm and landed at the
    // line END instead.
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(400.0), None);
    buffer.set_wrap(Wrap::None);
    buffer.set_text("hi", &Attrs::new(), Shaping::Advanced, None);
    buffer.lines[0].set_align(Some(Align::Right));
    buffer.shape_until_scroll(&mut font_system, false);

    let run = buffer
        .layout_runs()
        .next()
        .expect("expected at least one layout run");
    let first_glyph_x = run.glyphs.first().expect("expected glyphs").x;
    assert!(
        first_glyph_x > 10.0,
        "the right-aligned line must start well right of x=0 (got {first_glyph_x})"
    );

    let cursor = buffer
        .hit(first_glyph_x / 2.0, 10.0)
        .expect("expected a hit");
    assert_eq!(
        cursor.index, 0,
        "a click in the leading gap of a right-aligned line must hit the line start"
    );
}

#[test]
fn click_before_a_flush_left_line_still_hits_its_start() {
    // Guard for the flush-left case the old `x < 0.0` did handle: a click at
    // negative x still resolves to the line start.
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(400.0), None);
    buffer.set_wrap(Wrap::None);
    buffer.set_text("hi", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);

    let cursor = buffer.hit(-5.0, 10.0).expect("expected a hit");
    assert_eq!(cursor.index, 0);
}
