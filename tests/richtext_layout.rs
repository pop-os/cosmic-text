use cosmic_text::{
    fontdb, Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping,
};

// Test for https://github.com/pop-os/cosmic-text/issues/364
//
// Empty lines at the start/end of a span should use that span's line height,
// not the buffer's default line height.
#[test]
fn empty_lines_use_span_metrics() {
    let mut font_system = FontSystem::new();

    let metrics = Metrics::new(32.0, 44.0);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);

    let attrs = Attrs::new();
    let small_attrs = attrs.clone().metrics(Metrics::relative(8.0, 1.2));

    // The empty lines from \n\n at start and end should use 8.0 * 1.2 = 9.6 line height.
    // All newlines are inside the small_attrs span so the empty lines are clearly within it.
    buffer.set_rich_text(
        [
            ("Before", attrs.clone()),
            ("\n\n\nSmall\n\n", small_attrs),
            ("After", attrs.clone()),
        ],
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.set_size(Some(500.0), Some(500.0));

    let line_heights: Vec<f32> = buffer.layout_runs().map(|run| run.line_height).collect();

    // line_heights should be:
    // [0] "Before"   -> 44.0 (buffer default)
    // [1] "" (empty)  -> 9.6  (small span metrics: 8.0 * 1.2)
    // [2] "" (empty)  -> 9.6  (small span metrics: 8.0 * 1.2)
    // [3] "Small"     -> 9.6  (small span metrics from glyphs)
    // [4] "" (empty)  -> 9.6  (small span metrics: 8.0 * 1.2)
    // [5] "After"     -> 44.0 (buffer default)
    assert_eq!(
        line_heights.len(),
        6,
        "expected 6 layout runs, got {}",
        line_heights.len()
    );
    assert!(
        (line_heights[0] - 44.0).abs() < 0.1,
        "line 0 should use buffer default: {}",
        line_heights[0]
    );
    assert!(
        (line_heights[1] - 9.6).abs() < 0.1,
        "line 1 (empty) should use span metrics: {}",
        line_heights[1]
    );
    assert!(
        (line_heights[2] - 9.6).abs() < 0.1,
        "line 2 (empty) should use span metrics: {}",
        line_heights[2]
    );
    assert!(
        (line_heights[4] - 9.6).abs() < 0.1,
        "line 4 (empty) should use span metrics: {}",
        line_heights[4]
    );
    assert!(
        (line_heights[5] - 44.0).abs() < 0.1,
        "line 5 should use buffer default: {}",
        line_heights[5]
    );
}

// Test for https://github.com/pop-os/cosmic-text/issues/530
#[test]
fn font_features_apply_within_a_word() {
    let mut font_db = fontdb::Database::new();
    font_db.load_font_data(std::fs::read("fonts/Inter-Regular.ttf").unwrap());
    let mut font_system = FontSystem::new_with_locale_and_db("en-US".into(), font_db);

    let normal = Attrs::new().family(Family::Name("Inter"));
    let mut features = FontFeatures::new();
    features.enable(FeatureTag::new(b"subs"));

    let subscript = normal.clone().font_features(features);

    // The id of "3" without the 'subs' feature
    let normal_id = glyph_ids(&mut font_system, "3", &normal)[0];
    // The id of "3" with the 'subs' feature
    let subscript_id = glyph_ids(&mut font_system, "3", &subscript)[0];
    assert_ne!(normal_id, subscript_id, "test font must support 'subs'");

    let mut buffer = Buffer::new(&mut font_system, Metrics::new(32.0, 40.0));
    buffer.set_rich_text(
        [("x", normal.clone()), ("3", subscript)],
        &normal,
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, false);
    let rich_text_ids: Vec<_> = buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.glyph_id))
        .collect();

    assert_eq!(
        rich_text_ids[1], subscript_id,
        "subscript '3' should have different glyph id than normal '3'"
    );
}

fn glyph_ids(font_system: &mut FontSystem, text: &str, attrs: &Attrs<'_>) -> Vec<u16> {
    let mut buffer = Buffer::new(font_system, Metrics::new(32.0, 40.0));
    buffer.set_text(text, attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.glyph_id))
        .collect()
}
