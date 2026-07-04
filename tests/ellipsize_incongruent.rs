use cosmic_text::{
    fontdb, Attrs, Buffer, Direction, Ellipsize, EllipsizeHeightLimit, FontSystem, Metrics,
    Shaping, Wrap,
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

fn max_glyph_extent(buffer: &Buffer) -> f32 {
    buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.x + g.w).collect::<Vec<_>>())
        .fold(0.0, f32::max)
}

#[test]
fn ellipsis_respects_the_width_when_the_line_opens_incongruent() {
    // Forced-LTR base with RTL-first content: the first span of the (fresh)
    // line runs against the base direction. get_glyph_start_end must measure
    // that word fully; if the fresh-line sentinel aliases span 0 / word 0, the
    // opening word measures 0 wide, the overflow check believes more content
    // fits than does, and the ellipsized line paints past the buffer width.
    let mut font_system = font_system();

    // First, the unconstrained extent of the full line.
    let mut probe = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    probe.set_wrap(Wrap::None);
    probe.set_direction(Direction::LeftToRight);
    probe.set_text("سلام hello world", &Attrs::new(), Shaping::Advanced, None);
    probe.shape_until_scroll(&mut font_system, false);
    let full_extent = max_glyph_extent(&probe);
    assert!(full_extent > 0.0, "the probe line must produce glyphs");

    // Now constrain to well under the full extent and ellipsize the end.
    let width = full_extent * 0.6;
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(width), None);
    buffer.set_wrap(Wrap::None);
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
    buffer.set_text("سلام hello world", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);

    let extent = max_glyph_extent(&buffer);
    assert!(
        extent <= width + 0.5,
        "the ellipsized line must stay within the buffer width: extent {extent} > width {width}"
    );

    // And it must still paint something — ellipsizing is not dropping the line.
    assert!(extent > 0.0, "the ellipsized line must produce glyphs");
}
