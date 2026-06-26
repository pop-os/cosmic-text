use cosmic_text::{fontdb, Attrs, Buffer, Direction, FontSystem, Metrics, Shaping, Wrap};

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

fn first_run_rtl(buffer: &Buffer) -> bool {
    buffer
        .layout_runs()
        .next()
        .expect("expected at least one layout run")
        .rtl
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
fn auto_detects_per_paragraph() {
    let mut font_system = font_system();
    // Default Auto behavior: direction follows the first strong character.
    let ltr = make_buffer(&mut font_system, "hello", Direction::Auto);
    assert!(!first_run_rtl(&ltr));

    let rtl = make_buffer(&mut font_system, "سلام", Direction::Auto);
    assert!(first_run_rtl(&rtl));
}

#[test]
fn forced_rtl_overrides_ltr_content() {
    let mut font_system = font_system();
    let buffer = make_buffer(&mut font_system, "hello", Direction::RightToLeft);
    assert!(first_run_rtl(&buffer),);
}

#[test]
fn forced_ltr_overrides_rtl_content() {
    let mut font_system = font_system();
    let buffer = make_buffer(&mut font_system, "سلام", Direction::LeftToRight);
    assert!(!first_run_rtl(&buffer),);
}

#[test]
fn forced_ltr_keeps_rtl_glyphs() {
    // a line whose content is entirely RTL must still produce glyphs
    // when the base direction is forced to LTR (incongruent span on the no-wrap path)
    let mut font_system = font_system();
    let buffer = make_buffer(&mut font_system, "سلام", Direction::LeftToRight);
    let run = buffer
        .layout_runs()
        .next()
        .expect("expected at least one layout run");
    assert!(
        !run.glyphs.is_empty(),
        "forced-LTR RTL line produced no glyphs"
    );
}

#[test]
fn force_ltr_overrides_first_strong_rtl() {
    let mut font_system = font_system();
    let buffer = make_buffer(&mut font_system, "   سلام", Direction::LeftToRight);
    assert!(!first_run_rtl(&buffer));
}

#[test]
fn force_ltr_overrides_weak() {
    let mut font_system = font_system();
    let mut buffer = make_buffer(&mut font_system, "   ()", Direction::LeftToRight);
    assert!(!first_run_rtl(&buffer));
    buffer.set_direction(Direction::RightToLeft);
    buffer.shape_until_scroll(&mut font_system, false);
    assert!(first_run_rtl(&buffer),);
}

#[test]
fn changing_direction_reshapes_cached_lines() {
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_text("hello", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);
    assert!(!first_run_rtl(&buffer));

    // Switching direction must invalidate the already-shaped line.
    buffer.set_direction(Direction::RightToLeft);
    buffer.shape_until_scroll(&mut font_system, false);
    assert!(first_run_rtl(&buffer));
}
