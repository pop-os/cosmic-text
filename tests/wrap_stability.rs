use cosmic_text::{
    fontdb, Align, Attrs, AttrsList, BidiParagraphs, Family, FontSystem, LayoutLine, ShapeLine,
    Shaping, Weight, Wrap,
};

// Test for https://github.com/pop-os/cosmic-text/issues/134
//
// Being able to get the same wrapping when feeding the measured width back into ShapeLine::layout
// as the new width limit is very useful for certain UI layout use cases.
#[test]
fn stable_wrap() {
    let font_size = 18.0;
    let attrs = AttrsList::new(
        Attrs::new()
            .family(Family::Name("FiraMono"))
            .weight(Weight::MEDIUM),
    );
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/FiraMono-Medium.ttf").unwrap();
    font_system.db_mut().load_font_data(font);

    let mut check_wrap = |text: &_, wrap, start_width| {
        let line = ShapeLine::new(&mut font_system, text, &attrs, Shaping::Advanced);

        let layout_unbounded = line.layout(font_size, start_width, wrap, Some(Align::Left));
        let max_width = layout_unbounded.iter().map(|l| l.w).fold(0.0, f32::max);
        let new_limit = f32::min(start_width, max_width);

        let layout_bounded = line.layout(font_size, new_limit, wrap, Some(Align::Left));
        let bounded_max_width = layout_bounded.iter().map(|l| l.w).fold(0.0, f32::max);

        // For debugging:
        // dbg_layout_lines(text, &layout_unbounded);
        // dbg_layout_lines(text, &layout_bounded);

        assert_eq!(
            (max_width, layout_unbounded.len()),
            (bounded_max_width, layout_bounded.len()),
            "Wrap \"{wrap:?}\" with text: \"{text}\"",
        );
        for (u, b) in layout_unbounded[1..].iter().zip(layout_bounded[1..].iter()) {
            assert_eq!(u.w, b.w, "Wrap {wrap:?} with text: \"{text}\"",);
        }
    };

    let hello_sample = std::fs::read_to_string("sample/hello.txt").unwrap();
    let cases = [
        "(6)  SomewhatBoringDisplayTransform",
        "",
        " ",
        "  ",
        "   ",
        "       ",
    ]
    .into_iter()
    // This has several cases where the line would wrap when the computed width was used as the
    // width limit.
    .chain(BidiParagraphs::new(&hello_sample));

    for text in cases {
        for wrap in [Wrap::Word, Wrap::Glyph] {
            for start_width in [f32::MAX, 80.0, 198.2132, 20.0, 4.0, 300.0] {
                check_wrap(text, wrap, start_width);
                let with_spaces = format!("{text}            ");
                check_wrap(&with_spaces, wrap, start_width);
                let with_spaces_2 = format!("{text}    ");
                check_wrap(&with_spaces_2, wrap, start_width);
            }
        }
    }
}

#[allow(dead_code)]
fn dbg_layout_lines(text: &str, lines: &[LayoutLine]) {
    for line in lines {
        let mut s = String::new();
        for glyph in line.glyphs.iter() {
            s.push_str(&text[glyph.start..glyph.end]);
        }
        println!("\"{s}\"");
    }
}
