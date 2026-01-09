use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};

#[test]
fn ligature_segmentation() {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/Inter-Regular.ttf").unwrap();
    font_system.db_mut().load_font_data(font);
    let metrics = Metrics::new(14.0, 20.0);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);

    buffer.set_text("|>", &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(false);

    let line = &buffer.lines[0];
    let shape = line.shape_opt().expect("ShapeLine not found");
    let span = &shape.spans[0];
    
    // The pipe character | is typically a line break opportunity.
    // This test ensures that our patch prevents splitting |> into separate words,
    // which would break ligature formation in fonts that support it.
    assert_eq!(
        span.words.len(),
        1,
        "Expected '|>' to be a single word (preserved for ligature), but found {} words.",
        span.words.len()
    );
}
