use super::{CacheKey, Font, FontLayoutGlyph, FontLayoutLine, FontLineIndex};

pub struct FontShapeGlyph<'a> {
    pub start: usize,
    pub end: usize,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub font: &'a Font<'a>,
    pub inner: swash::GlyphId,
}

impl<'a> FontShapeGlyph<'a> {
    fn layout(&self, font_size: i32, x: f32, y: f32) -> FontLayoutGlyph<'a> {
        let x_offset = font_size as f32 * self.x_offset;
        let y_offset = font_size as f32 * self.y_offset;
        let x_advance = font_size as f32 * self.x_advance;

        let inner = CacheKey::new(self.inner, font_size, (x + x_offset, y - y_offset));
        FontLayoutGlyph {
            start: self.start,
            end: self.end,
            x: x,
            w: x_advance,
            font: self.font,
            inner,
        }
    }
}

pub struct FontShapeWord<'a> {
    pub blank: bool,
    pub glyphs: Vec<FontShapeGlyph<'a>>,
}

pub struct FontShapeSpan<'a> {
    pub rtl: bool,
    pub words: Vec<FontShapeWord<'a>>,
}

pub struct FontShapeLine<'a> {
    pub line_i: FontLineIndex,
    pub rtl: bool,
    pub spans: Vec<FontShapeSpan<'a>>,
}

impl<'a> FontShapeLine<'a> {
    pub fn layout(
        &self,
        font_size: i32,
        line_width: i32,
        layout_lines: &mut Vec<FontLayoutLine<'a>>,
        mut layout_i: usize,
    ) {
        let mut push_line = true;
        let mut glyphs = Vec::new();

        let start_x = if self.rtl { line_width as f32 } else { 0.0 };
        let end_x = if self.rtl { 0.0 } else { line_width as f32 };
        let mut x = start_x;
        let mut y = 0.0;
        for span in self.spans.iter() {
            //TODO: improve performance!
            let mut word_ranges = Vec::new();
            if self.rtl != span.rtl {
                let mut fit_x = x;
                let mut fitting_end = span.words.len();
                for i in (0..span.words.len()).rev() {
                    let word = &span.words[i];

                    let mut word_size = 0.0;
                    for glyph in word.glyphs.iter() {
                        word_size += font_size as f32 * glyph.x_advance;
                    }

                    let wrap = if self.rtl {
                        fit_x - word_size < end_x
                    } else {
                        fit_x + word_size > end_x
                    };

                    if wrap {
                        let mut fitting_start = i + 1;
                        while fitting_start < fitting_end {
                            if span.words[fitting_start].blank {
                                fitting_start += 1;
                            } else {
                                break;
                            }
                        }
                        word_ranges.push((fitting_start..fitting_end, true));
                        fitting_end = i + 1;

                        fit_x = start_x;
                    }

                    if self.rtl {
                        fit_x -= word_size;
                    } else {
                        fit_x += word_size;
                    }
                }
                if !word_ranges.is_empty() {
                    while fitting_end > 0 {
                        if span.words[fitting_end - 1].blank {
                            fitting_end -= 1;
                        } else {
                            break;
                        }
                    }
                }
                word_ranges.push((0..fitting_end, false));
            } else {
                let mut fit_x = x;
                let mut fitting_start = 0;
                for i in 0..span.words.len() {
                    let word = &span.words[i];

                    let mut word_size = 0.0;
                    for glyph in word.glyphs.iter() {
                        word_size += font_size as f32 * glyph.x_advance;
                    }

                    let wrap = if self.rtl {
                        fit_x - word_size < end_x
                    } else {
                        fit_x + word_size > end_x
                    };

                    if wrap {
                        //TODO: skip blanks
                        word_ranges.push((fitting_start..i, true));
                        fitting_start = i;

                        fit_x = start_x;
                    }

                    if self.rtl {
                        fit_x -= word_size;
                    } else {
                        fit_x += word_size;
                    }
                }
                word_ranges.push((fitting_start..span.words.len(), false));
            }

            for (range, wrap) in word_ranges {
                for word in span.words[range].iter() {
                    let mut word_size = 0.0;
                    for glyph in word.glyphs.iter() {
                        word_size += font_size as f32 * glyph.x_advance;
                    }

                    //TODO: make wrapping optional
                    let wrap = if self.rtl {
                        x - word_size < end_x
                    } else {
                        x + word_size > end_x
                    };
                    if wrap && !glyphs.is_empty() {
                        let mut glyphs_swap = Vec::new();
                        std::mem::swap(&mut glyphs, &mut glyphs_swap);
                        layout_lines.insert(
                            layout_i,
                            FontLayoutLine {
                                line_i: self.line_i,
                                glyphs: glyphs_swap,
                            },
                        );
                        layout_i += 1;

                        x = start_x;
                        y = 0.0;
                    }

                    for glyph in word.glyphs.iter() {
                        let x_advance = font_size as f32 * glyph.x_advance;
                        let y_advance = font_size as f32 * glyph.y_advance;

                        if self.rtl {
                            x -= x_advance
                        }

                        glyphs.push(glyph.layout(font_size, x, y));
                        push_line = true;

                        if !self.rtl {
                            x += x_advance;
                        }
                        y += y_advance;
                    }
                }

                if wrap {
                    let mut glyphs_swap = Vec::new();
                    std::mem::swap(&mut glyphs, &mut glyphs_swap);
                    layout_lines.insert(
                        layout_i,
                        FontLayoutLine {
                            line_i: self.line_i,
                            glyphs: glyphs_swap,
                        },
                    );
                    layout_i += 1;

                    x = start_x;
                    y = 0.0;
                }
            }
        }

        if push_line {
            layout_lines.insert(
                layout_i,
                FontLayoutLine {
                    line_i: self.line_i,
                    glyphs,
                },
            );
        }
    }
}
