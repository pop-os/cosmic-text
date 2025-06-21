use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Wrap};

#[test]
fn wrap_word_fallback() {
    // Create a new font database and font system
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());

    // Load the Inter font from file — this registers the family name
    font_system
        .db_mut()
        .load_font_file("fonts/Inter-Regular.ttf")
        .expect("Failed to load Inter-Regular.ttf");

    // Setup text layout metrics
    let metrics = Metrics::new(14.0, 20.0);

    // Create and borrow the buffer
    let mut buffer_base = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer_base.borrow_with(&mut font_system);

    // Enable word or glyph wrapping
    buffer.set_wrap(Wrap::WordOrGlyph);

    // Use the Inter font family — this will now be resolved properly
    let attrs = Attrs::new().family(Family::Name("Inter"));

    // Set the test string and shape it
    buffer.set_text(
        "Lorem ipsum dolor sit amet, qui minim labore adipisicing minim sint cillum sint consectetur cupidatat.",
        &attrs,
        Shaping::Advanced,
    );

    buffer.set_size(Some(50.0), Some(1000.0));
    buffer.shape_until_scroll(false);

    // Measure the longest line width
    let measured_width = measure(&buffer);
    let buffer_width = buffer.size().0.expect("Buffer width is not set");

    // Ensure the layout does not exceed the buffer width
    assert!(
        measured_width <= buffer_width,
        "Measured width is larger than buffer width\n{} <= {}",
        measured_width,
        buffer_width
    );
}

// Measures the max line width from the buffer
fn measure(buffer: &Buffer) -> f32 {
    buffer
        .layout_runs()
        .fold(0.0, |max_width, run| max_width.max(run.line_w))
}
