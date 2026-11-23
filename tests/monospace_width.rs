use cosmic_text::{fontdb, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};

fn load_fira_mono() -> FontSystem {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/FiraMono-Medium.ttf").unwrap();
    font_system.db_mut().load_font_data(font);
    font_system
}

#[test]
fn combining_mark_clusters_advance_once_in_monospace_layout() {
    let mut font_system = load_fira_mono();
    let metrics = Metrics::new(10.0, 14.0);
    let attrs = Attrs::new()
        .family(Family::Name("FiraMono"))
        .weight(Weight::MEDIUM);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_monospace_width(Some(10.0));
    buffer.set_text(
        "|x\u{0301}x\u{0301}x\u{0301}x\u{0301}x\u{0301}|END\n\
         |x\u{0302}x\u{0302}x\u{0302}x\u{0302}x\u{0302}|END\n\
         |x\u{0301}\u{0302}x\u{0301}\u{0302}x\u{0301}\u{0302}x\u{0301}\u{0302}x\u{0301}\u{0302}|END",
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(false);

    let widths = buffer
        .layout_runs()
        .map(|run| run.line_w)
        .collect::<Vec<_>>();

    assert_eq!(widths, vec![100.0, 100.0, 100.0]);
}

#[test]
fn simple_emoji_presentation_selector_does_not_widen_base_symbol() {
    let mut font_system = load_fira_mono();
    let metrics = Metrics::new(10.0, 14.0);
    let attrs = Attrs::new()
        .family(Family::Name("FiraMono"))
        .weight(Weight::MEDIUM);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_monospace_width(Some(10.0));
    buffer.set_text(
        "|\u{2665}\u{2665}\u{2665}\u{2665}\u{2665}|END\n\
         |\u{2764}\u{FE0F}\u{2764}\u{FE0F}\u{2764}\u{FE0F}\u{2764}\u{FE0F}\u{2764}\u{FE0F}|END\n\
         |\u{2708}\u{2708}\u{2708}\u{2708}\u{2708}|END\n\
         |\u{2708}\u{FE0F}\u{2708}\u{FE0F}\u{2708}\u{FE0F}\u{2708}\u{FE0F}\u{2708}\u{FE0F}|END",
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(false);

    let widths = buffer
        .layout_runs()
        .map(|run| run.line_w)
        .collect::<Vec<_>>();

    assert_eq!(widths, vec![100.0, 100.0, 100.0, 100.0]);
}
