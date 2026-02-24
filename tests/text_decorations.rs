use common::DrawTestCfg;
use cosmic_text::{Attrs, Color, Family, UnderlineStyle};

mod common;

fn base_attrs() -> Attrs<'static> {
    Attrs::new().family(Family::Name("Noto Sans"))
}

/// Single test covering all decoration variants:
/// - Single underline, double underline
/// - Strikethrough, overline
/// - Colored underline (red), colored strikethrough (cyan)
/// - All three combined on one span
/// - Plain text (no decoration) between spans
#[test]
fn test_text_decorations() {
    let base = base_attrs();
    let red = Color::rgb(0xFF, 0x00, 0x00);
    let cyan = Color::rgb(0x00, 0xFF, 0xFF);

    DrawTestCfg::new("text_decorations")
        .font_size(20., 26.)
        .font_attrs(base.clone())
        .rich_text(vec![
            ("Under ", base.clone().underline(UnderlineStyle::Single)),
            ("Double ", base.clone().underline(UnderlineStyle::Double)),
            ("Strike ", base.clone().strikethrough()),
            ("Over ", base.clone().overline()),
            (
                "RedUl ",
                base.clone()
                    .underline(UnderlineStyle::Single)
                    .underline_color(red),
            ),
            (
                "CyanSt ",
                base.clone().strikethrough().strikethrough_color(cyan),
            ),
            (
                "All",
                base.clone()
                    .underline(UnderlineStyle::Single)
                    .strikethrough()
                    .overline(),
            ),
            (" Plain", base),
        ])
        .canvas(600, 50)
        .validate_text_rendering();
}
