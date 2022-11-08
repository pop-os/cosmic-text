// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::mem;
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

use crate::{AttrsList, CacheKey, Color, Font, FontSystem, LayoutGlyph, LayoutLine};
use crate::fallback::FontFallbackIter;

fn shape_fallback(
    font: &Font,
    line: &str,
    attrs_list: &AttrsList,
    start_run: usize,
    end_run: usize,
    span_rtl: bool,
) -> (Vec<ShapeGlyph>, Vec<usize>) {
    let run = &line[start_run..end_run];

    let font_scale = font.rustybuzz.units_per_em() as f32;

    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.set_direction(if span_rtl {
        rustybuzz::Direction::RightToLeft
    } else {
        rustybuzz::Direction::LeftToRight
    });
    buffer.push_str(run);
    buffer.guess_segment_properties();

    let rtl = match buffer.direction() {
        rustybuzz::Direction::RightToLeft => true,
        //TODO: other directions?
        _ => false,
    };
    assert_eq!(rtl, span_rtl);

    let glyph_buffer = rustybuzz::shape(&font.rustybuzz, &[], buffer);
    let glyph_infos = glyph_buffer.glyph_infos();
    let glyph_positions = glyph_buffer.glyph_positions();

    let mut missing = Vec::new();
    let mut glyphs = Vec::with_capacity(glyph_infos.len());
    for (info, pos) in glyph_infos.iter().zip(glyph_positions.iter()) {
        let x_advance = pos.x_advance as f32 / font_scale;
        let y_advance = pos.y_advance as f32 / font_scale;
        let x_offset = pos.x_offset as f32 / font_scale;
        let y_offset = pos.y_offset as f32 / font_scale;

        let start_glyph = start_run + info.cluster as usize;

        //println!("  {:?} {:?}", info, pos);
        if info.glyph_id == 0 {
            missing.push(start_glyph);
        }

        glyphs.push(ShapeGlyph {
            start: start_glyph,
            end: end_run, // Set later
            x_advance,
            y_advance,
            x_offset,
            y_offset,
            font_id: font.info.id,
            glyph_id: info.glyph_id.try_into().unwrap(),
            color_opt: None,
        });
    }

    // Adjust end of glyphs
    if rtl {
        for i in 1..glyphs.len() {
            let next_start = glyphs[i - 1].start;
            let next_end = glyphs[i - 1].end;
            let prev = &mut glyphs[i];
            if prev.start == next_start {
                prev.end = next_end;
            } else {
                prev.end = next_start;
            }
        }
    } else {
        for i in (1..glyphs.len()).rev() {
            let next_start = glyphs[i].start;
            let next_end = glyphs[i].end;
            let prev = &mut glyphs[i - 1];
            if prev.start == next_start {
                prev.end = next_end;
            } else {
                prev.end = next_start;
            }
        }
    }

    // Set color
    //TODO: these attributes should not be related to shaping
    for glyph in glyphs.iter_mut() {
        let attrs = attrs_list.get_span(glyph.start..glyph.end);
        glyph.color_opt = attrs.color_opt;
    }

    (glyphs, missing)
}

fn shape_run<'a>(
    font_system: &'a mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    start_run: usize,
    end_run: usize,
    span_rtl: bool,
) -> Vec<ShapeGlyph> {
    //TODO: use smallvec?
    let mut scripts = Vec::new();
    for c in line[start_run..end_run].chars() {
        match c.script() {
            Script::Common |
            Script::Inherited |
            Script::Latin |
            Script::Unknown => (),
            script => if ! scripts.contains(&script) {
                scripts.push(script);
            },
        }
    }

    log::trace!(
        "      Run {:?}: '{}'",
        scripts,
        &line[start_run..end_run],
    );

    let attrs = attrs_list.get_span(start_run..end_run);

    let font_matches = font_system.get_font_matches(attrs);

    let default_families = [font_matches.default_family.as_str()];
    let mut font_iter = FontFallbackIter::new(
        &font_matches.fonts,
        &default_families,
        scripts,
        font_matches.locale
    );

    let (mut glyphs, mut missing) = shape_fallback(
        font_iter.next().unwrap(),
        line,
        attrs_list,
        start_run,
        end_run,
        span_rtl,
    );

    //TODO: improve performance!
    while !missing.is_empty() {
        let font = match font_iter.next() {
            Some(some) => some,
            None => break,
        };

        log::trace!("Evaluating fallback with font '{}'", font.info.family);
        let (mut fb_glyphs, fb_missing) = shape_fallback(
            font,
            line,
            attrs_list,
            start_run,
            end_run,
            span_rtl,
        );

        // Insert all matching glyphs
        let mut fb_i = 0;
        while fb_i < fb_glyphs.len() {
            let start = fb_glyphs[fb_i].start;
            let end = fb_glyphs[fb_i].end;

            // Skip clusters that are not missing, or where the fallback font is missing
            if !missing.contains(&start) || fb_missing.contains(&start) {
                fb_i += 1;
                continue;
            }

            let mut missing_i = 0;
            while missing_i < missing.len() {
                if missing[missing_i] >= start && missing[missing_i] < end {
                    // println!("No longer missing {}", missing[missing_i]);
                    missing.remove(missing_i);
                } else {
                    missing_i += 1;
                }
            }

            // Find prior glyphs
            let mut i = 0;
            while i < glyphs.len() {
                if glyphs[i].start >= start && glyphs[i].end <= end {
                    break;
                } else {
                    i += 1;
                }
            }

            // Remove prior glyphs
            while i < glyphs.len() {
                if glyphs[i].start >= start && glyphs[i].end <= end {
                    let _glyph = glyphs.remove(i);
                    // log::trace!("Removed {},{} from {}", _glyph.start, _glyph.end, i);
                } else {
                    break;
                }
            }

            while fb_i < fb_glyphs.len() {
                if fb_glyphs[fb_i].start >= start && fb_glyphs[fb_i].end <= end {
                    let fb_glyph = fb_glyphs.remove(fb_i);
                    // log::trace!("Insert {},{} from font {} at {}", fb_glyph.start, fb_glyph.end, font_i, i);
                    glyphs.insert(i, fb_glyph);
                    i += 1;
                } else {
                    break;
                }
            }
        }
    }

    // Debug missing font fallbacks
    font_iter.check_missing(&line[start_run..end_run]);

    /*
    for glyph in glyphs.iter() {
        log::trace!("'{}': {}, {}, {}, {}", &line[glyph.start..glyph.end], glyph.x_advance, glyph.y_advance, glyph.x_offset, glyph.y_offset);
    }
    */

    glyphs
}

/// A shaped glyph
pub struct ShapeGlyph {
    pub start: usize,
    pub end: usize,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    pub color_opt: Option<Color>,
}

impl ShapeGlyph {
    fn layout(&self, font_size: i32, x: f32, y: f32, rtl: bool) -> LayoutGlyph {
        let x_offset = font_size as f32 * self.x_offset;
        let y_offset = font_size as f32 * self.y_offset;
        let x_advance = font_size as f32 * self.x_advance;

        let (cache_key, x_int, y_int) = CacheKey::new(
            self.font_id,
            self.glyph_id,
            font_size,
            (x + x_offset, y - y_offset)
        );
        LayoutGlyph {
            start: self.start,
            end: self.end,
            x,
            w: x_advance,
            rtl,
            cache_key,
            x_int,
            y_int,
            color_opt: self.color_opt,
        }
    }
}

/// A shaped word (for word wrapping)
pub struct ShapeWord {
    pub blank: bool,
    pub glyphs: Vec<ShapeGlyph>,
}

impl ShapeWord {
    pub fn new<'a>(
        font_system: &'a mut FontSystem,
        line: &str,
        attrs_list: &AttrsList,
        start_word: usize,
        end_word: usize,
        span_rtl: bool,
        blank: bool,
    ) -> Self {
        let word = &line[start_word..end_word];

        log::trace!(
            "      Word{}: '{}'",
            if blank { " BLANK" } else { "" },
            word
        );

        let mut glyphs = Vec::new();

        let mut start_run = start_word;
        let mut attrs = attrs_list.defaults();
        for (egc_i, egc) in word.grapheme_indices(true) {
            let start_egc = start_word + egc_i;
            let end_egc = start_egc + egc.len();
            let attrs_egc = attrs_list.get_span(start_egc..end_egc);
            if ! attrs.compatible(&attrs_egc) {
                //TODO: more efficient
                glyphs.append(&mut shape_run(
                    font_system,
                    line,
                    attrs_list,
                    start_run,
                    start_egc,
                    span_rtl
                ));

                start_run = start_egc;
                attrs = attrs_egc;
            }
        }
        if start_run < end_word {
            //TODO: more efficient
            glyphs.append(&mut shape_run(
                font_system,
                line,
                attrs_list,
                start_run,
                end_word,
                span_rtl
            ));
        }

        Self { blank, glyphs }
    }
}

/// A shaped span (for bidirectional processing)
pub struct ShapeSpan {
    pub rtl: bool,
    pub words: Vec<ShapeWord>,
}

impl ShapeSpan {
    pub fn new<'a>(
        font_system: &'a mut FontSystem,
        line: &str,
        attrs_list: &AttrsList,
        start_span: usize,
        end_span: usize,
        line_rtl: bool,
        span_rtl: bool,
    ) -> Self {
        let span = &line[start_span..end_span];

        log::trace!(
            "  Span {}: '{}'",
            if span_rtl { "RTL" } else { "LTR" },
            span
        );

        let mut words = Vec::new();

        let mut start_word = 0;
        for (end_lb, _) in unicode_linebreak::linebreaks(span) {
            let mut start_lb = end_lb;
            for (i, c) in span[start_word..end_lb].char_indices() {
                if start_word + i == end_lb {
                    break;
                } else if c.is_whitespace() {
                    start_lb = start_word + i;
                }
            }
            if start_word < start_lb {
                words.push(ShapeWord::new(
                    font_system,
                    line,
                    attrs_list,
                    start_span + start_word,
                    start_span + start_lb,
                    span_rtl,
                    false,
                ));
            }
            if start_lb < end_lb {
                words.push(ShapeWord::new(
                    font_system,
                    line,
                    attrs_list,
                    start_span + start_lb,
                    start_span + end_lb,
                    span_rtl,
                    true,
                ));
            }
            start_word = end_lb;
        }

        // Reverse glyphs in RTL lines
        if line_rtl {
            for word in words.iter_mut() {
                word.glyphs.reverse();
            }
        }

        // Reverse words in spans that do not match line direction
        if line_rtl != span_rtl {
            words.reverse();
        }

        ShapeSpan {
            rtl: span_rtl,
            words,
        }
    }
}

/// A shaped line (or paragraph)
pub struct ShapeLine {
    pub rtl: bool,
    pub spans: Vec<ShapeSpan>,
}

impl ShapeLine {
    pub fn new<'a>(
        font_system: &'a mut FontSystem,
        line: &str,
        attrs_list: &AttrsList
    ) -> Self {
        let mut spans = Vec::new();

        let bidi = unicode_bidi::BidiInfo::new(line, None);
        let rtl = if bidi.paragraphs.is_empty() {
            false
        } else {
            assert_eq!(bidi.paragraphs.len(), 1);
            let para_info = &bidi.paragraphs[0];
            let line_rtl = para_info.level.is_rtl();

            log::trace!("Line {}: '{}'", if line_rtl { "RTL" } else { "LTR" }, line);

            let paragraph = unicode_bidi::Paragraph::new(&bidi, para_info);

            let mut start = 0;
            let mut span_rtl = line_rtl;
            for i in paragraph.para.range.clone() {
                let next_rtl = paragraph.info.levels[i].is_rtl();
                if span_rtl != next_rtl {
                    spans.push(ShapeSpan::new(
                        font_system,
                        line,
                        attrs_list,
                        start,
                        i,
                        line_rtl,
                        span_rtl
                    ));
                    span_rtl = next_rtl;
                    start = i;
                }
            }
            spans.push(ShapeSpan::new(
                font_system,
                line,
                attrs_list,
                start,
                line.len(),
                line_rtl,
                span_rtl
            ));

            line_rtl
        };

        Self { rtl, spans }
    }

    pub fn layout(
        &self,
        font_size: i32,
        line_width: i32,
        wrap_simple: bool,
    ) -> Vec<LayoutLine> {
        let mut layout_lines = Vec::with_capacity(1);

        let mut push_line = true;
        let mut glyphs = Vec::new();

        let start_x = if self.rtl { line_width as f32 } else { 0.0 };
        let end_x = if self.rtl { 0.0 } else { line_width as f32 };
        let mut x = start_x;
        let mut y = 0.0;
        for span in self.spans.iter() {
            //TODO: improve performance!
            let mut word_ranges = Vec::new();
            if wrap_simple {
                word_ranges.push((0..span.words.len(), false));
            } else if self.rtl != span.rtl {
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

                    let word_wrap = if self.rtl {
                        x - word_size < end_x
                    } else {
                        x + word_size > end_x
                    };
                    if word_wrap && !wrap_simple && !glyphs.is_empty() {
                        let mut glyphs_swap = Vec::new();
                        mem::swap(&mut glyphs, &mut glyphs_swap);
                        layout_lines.push(
                            LayoutLine {
                                glyphs: glyphs_swap,
                            },
                        );

                        x = start_x;
                        y = 0.0;
                    }

                    for glyph in word.glyphs.iter() {
                        let x_advance = font_size as f32 * glyph.x_advance;
                        let y_advance = font_size as f32 * glyph.y_advance;

                        //TODO: fix ordering when in bidi text
                        let glyph_wrap = if self.rtl {
                            x - x_advance < end_x
                        } else {
                            x + x_advance > end_x
                        };

                        if glyph_wrap {
                            let mut glyphs_swap = Vec::new();
                            mem::swap(&mut glyphs, &mut glyphs_swap);
                            layout_lines.push(
                                LayoutLine {
                                    glyphs: glyphs_swap,
                                },
                            );

                            x = start_x;
                            y = 0.0;
                        }

                        if self.rtl {
                            x -= x_advance
                        }

                        glyphs.push(glyph.layout(font_size, x, y, span.rtl));
                        push_line = true;

                        if !self.rtl {
                            x += x_advance;
                        }
                        y += y_advance;
                    }
                }

                if wrap {
                    let mut glyphs_swap = Vec::new();
                    mem::swap(&mut glyphs, &mut glyphs_swap);
                    layout_lines.push(
                        LayoutLine {
                            glyphs: glyphs_swap,
                        },
                    );

                    x = start_x;
                    y = 0.0;
                }
            }
        }

        if push_line {
            layout_lines.push(
                LayoutLine {
                    glyphs,
                },
            );
        }

        layout_lines
    }
}
