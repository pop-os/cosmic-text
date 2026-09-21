use std::path::PathBuf;

use cosmic_text::{fontdb, Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

/// A word whose prefix carries an explicit (non-default) attribute span while
/// the tail falls back to the default attrs must not be shaped entirely with
/// the prefix's font. The ASCII fast path in `ShapeWord::build` only inspects
/// the explicit spans, so the default tail is an uncovered gap that must still
/// be checked before taking the fast path.
///
/// Repro: word "Hello", prefix "He" = Fira Mono (explicit span), tail "llo" =
/// default (Inter, a gap). The tail must be shaped with Inter.
#[test]
fn fast_path_respects_default_tail_after_styled_prefix() {
    let repo_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let fonts_path = PathBuf::from(&repo_dir).join("fonts");

    // Empty db with only Inter and Fira Mono so font fallback is deterministic.
    let mut db = fontdb::Database::new();
    db.load_font_data(std::fs::read(fonts_path.join("Inter-Regular.ttf")).unwrap());
    db.load_font_data(std::fs::read(fonts_path.join("FiraMono-Medium.ttf")).unwrap());
    let mut font_system = FontSystem::new_with_locale_and_db("en-US".into(), db);

    // Default = Inter, prefix = Fira Mono. Two distinct physical faces so a
    // mis-shaped tail shows up as a font_id mismatch, independent of weight.
    let default_attrs = Attrs::new().family(Family::Name("Inter"));
    let prefix_attrs = Attrs::new().family(Family::Name("Fira Mono"));

    let metrics = Metrics::new(16.0, 20.0);
    let mut buffer = Buffer::new(&mut font_system, metrics);

    let glyphs: Vec<(usize, fontdb::ID)>;
    {
        let mut buffer = buffer.borrow_with(&mut font_system);
        buffer.set_size(Some(300.0), Some(100.0));
        // `set_rich_text` only records a span when its attrs differ from the
        // list defaults, so passing `default_attrs` for "llo" leaves an
        // uncovered gap at bytes 2..5 (Inter) rather than an explicit span —
        // exactly the default-attrs tail the fast-path gap check guards.
        buffer.set_rich_text(
            [("He", prefix_attrs), ("llo", default_attrs.clone())],
            &default_attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(true);
        glyphs = buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter().map(|g| (g.start, g.font_id)))
            .collect();
    }

    assert!(!glyphs.is_empty(), "no glyphs were produced");

    // "He" is 2 ASCII bytes (offsets 0..2); "llo" follows at 2..5.
    // `LayoutGlyph::start` is the byte offset of the cluster in the line.
    for (start, id) in &glyphs {
        let face = font_system.db().face(*id).unwrap();
        let family = &face.families[0].0;
        if *start < 2 {
            assert!(
                family.contains("Fira"),
                "prefix glyph at {start} used \"{family}\", expected Fira Mono"
            );
        } else {
            assert!(
                family.contains("Inter"),
                "tail glyph at {start} shaped as \"{family}\", expected Inter (default tail leaked the prefix font)"
            );
        }
    }
}
