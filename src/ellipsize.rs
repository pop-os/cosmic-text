use crate::{
    Attrs, AttrsList, FontSystem, ShapeGlyph, ShapeSpan, ShapeWord, Shaping, VisualLine, VlRange,
};

#[derive(Clone, Debug)]
pub(crate) struct EllipsisCache {
    pub(crate) glyphs: Vec<ShapeGlyph>,
}

#[derive(Debug)]
pub(crate) struct StartEllipsizePlan {
    pub(crate) ranges: Vec<VlRange>,
    pub(crate) ellipsis: Vec<ShapeGlyph>,
    pub(crate) ellipsis_level: unicode_bidi::Level,
    pub(crate) line_w: f32,
}

pub(crate) fn plan_start_ellipsize(
    visual_line: &VisualLine,
    spans: &[ShapeSpan],
    ellipsis_opt: Option<&EllipsisCache>,
    font_size: f32,
    goal_width: f32,
) -> Option<StartEllipsizePlan> {
    if visual_line.w <= goal_width {
        return None;
    }
    let ellipsis_cache = ellipsis_opt?;
    if ellipsis_cache.glyphs.is_empty() {
        return None;
    }
    let ellipsis_w: f32 = ellipsis_cache
        .glyphs
        .iter()
        .map(|g| g.width(font_size))
        .sum();
    if ellipsis_w <= 0.0 {
        return None;
    }
    let ellipsis_glyphs = ellipsis_cache.glyphs.clone();
    let required_remove = (visual_line.w + ellipsis_w - goal_width).max(0.0);
    if required_remove <= 0.0 {
        return None;
    }
    let mut removed_w = 0.0;
    let mut removed_start: Option<usize> = None;
    let mut removed_end: Option<usize> = None;
    let mut min_level: Option<unicode_bidi::Level> = None;
    // Find the cut point in logical order (visual_line.ranges is logical order)
    let mut cut_range_index: Option<usize> = None;
    let mut cut_word: usize = 0;
    let mut cut_glyph: usize = 0;
    'outer: for (
        range_index,
        &(span_index, (starting_word, starting_glyph), (ending_word, ending_glyph)),
    ) in visual_line.ranges.iter().enumerate()
    {
        let span = &spans[span_index];
        for word_i in starting_word..ending_word + usize::from(ending_glyph != 0) {
            let word = &span.words[word_i];
            let (start_i, end_i) = match (word_i == starting_word, word_i == ending_word) {
                (false, false) => (0, word.glyphs.len()),
                (true, false) => (starting_glyph, word.glyphs.len()),
                (false, true) => (0, ending_glyph),
                (true, true) => (starting_glyph, ending_glyph),
            };
            let mut g = start_i;
            while g < end_i {
                let cluster_start = word.glyphs[g].start;
                let cluster_end = word.glyphs[g].end;
                let mut cluster_w = 0.0;
                let mut g_end = g;
                while g_end < end_i
                    && word.glyphs[g_end].start == cluster_start
                    && word.glyphs[g_end].end == cluster_end
                {
                    cluster_w += word.glyphs[g_end].width(font_size);
                    g_end += 1;
                }
                removed_w += cluster_w;
                removed_start.get_or_insert(cluster_start);
                removed_end = Some(cluster_end);
                min_level = Some(min_level.map_or(span.level, |lvl| lvl.min(span.level)));
                if removed_w >= required_remove {
                    cut_range_index = Some(range_index);
                    cut_word = word_i;
                    cut_glyph = g_end;
                    break 'outer;
                }
                g = g_end;
            }
        }
    }
    let cut_range_index = cut_range_index?;
    let min_level = min_level?;
    let removed_end = removed_end?;
    // Build adjusted ranges (drop prefix)
    let mut new_ranges = Vec::with_capacity(visual_line.ranges.len());
    for (i, range) in visual_line.ranges.iter().enumerate() {
        if i < cut_range_index {
            continue;
        }
        if i == cut_range_index {
            // normalize start if glyph index is past word end
            let mut start_word = cut_word;
            let mut start_glyph = cut_glyph;
            if let Some(span) = spans.get(range.0) {
                if let Some(word) = span.words.get(start_word) {
                    if start_glyph >= word.glyphs.len() {
                        start_word += 1;
                        start_glyph = 0;
                    }
                }
            }
            new_ranges.push((range.0, (start_word, start_glyph), range.2));
        } else {
            new_ranges.push(*range);
        }
    }
    let new_w = visual_line.w - removed_w + ellipsis_w;
    // Set ellipsis glyphs to cover the removed range
    let mut ellipsis = ellipsis_glyphs;
    if let Some(start) = removed_start {
        for g in &mut ellipsis {
            g.start = start;
            g.end = removed_end;
        }
    }
    Some(StartEllipsizePlan {
        ranges: new_ranges,
        ellipsis,
        ellipsis_level: min_level,
        line_w: new_w,
    })
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
