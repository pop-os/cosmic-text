use crate::{Attrs, AttrsList, FontSystem, ShapeGlyph, ShapeWord, Shaping};

#[derive(Clone, Debug)]
pub(crate) struct EllipsisCache {
    pub(crate) glyphs: Vec<ShapeGlyph>,
}

pub(crate) fn shape_ellipsis(
    font_system: &mut FontSystem,
    attrs: &Attrs,
    shaping: Shaping,
    span_rtl: bool,
) -> Vec<ShapeGlyph> {
    let attrs_list = AttrsList::new(attrs);
    let level = if span_rtl {
        unicode_bidi::Level::rtl()
    } else {
        unicode_bidi::Level::ltr()
    };
    let word = ShapeWord::new(
        font_system,
        "\u{2026}", // TODO: maybe do CJK ellipsis
        &attrs_list,
        0.."\u{2026}".len(),
        level,
        false,
        shaping,
    );
    let mut glyphs = word.glyphs;

    // did we fail to shape it?
    if glyphs.is_empty() || glyphs.iter().all(|g| g.glyph_id == 0) {
        let fallback = ShapeWord::new(
            font_system,
            "...",
            &attrs_list,
            0.."...".len(),
            level,
            false,
            shaping,
        );
        glyphs = fallback.glyphs;
    }
    glyphs
}
