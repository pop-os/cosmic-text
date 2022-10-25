// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{CacheKey, FontMatches};

pub struct FontLayoutGlyph {
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub w: f32,
    pub rtl: bool,
    pub cache_key: CacheKey,
    pub x_int: i32,
    pub y_int: i32,
}

pub struct FontLayoutLine {
    pub rtl: bool,
    pub glyphs: Vec<FontLayoutGlyph>,
}

impl FontLayoutLine {
    pub fn draw<F: FnMut(i32, i32, u32)>(&self, matches: &FontMatches<'_>, base: u32, mut f: F) {
        for glyph in self.glyphs.iter() {
            use swash::scale::{Render, Source, StrikeWith};
            use swash::zeno::{Format, Vector};

            let font = match matches.get_font(&glyph.cache_key.font_id) {
                Some(some) => some,
                None => {
                    log::warn!("did not find font {:?}", glyph.cache_key.font_id);
                    continue;
                },
            };

            let mut cache = font.cache.lock().unwrap();

            let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

            let image_opt = cache.entry(cache_key).or_insert_with(|| {
                let mut scale_context = font.scale_context.lock().unwrap();

                // Build the scaler
                let mut scaler = scale_context
                    .builder(font.swash)
                    .size(cache_key.font_size as f32)
                    .hint(true)
                    .build();

                // Compute the fractional offset-- you'll likely want to quantize this
                // in a real renderer
                let offset =
                    Vector::new(cache_key.x_bin.as_float(), cache_key.y_bin.as_float());

                // Select our source order
                Render::new(&[
                    // Color outline with the first palette
                    Source::ColorOutline(0),
                    // Color bitmap with best fit selection mode
                    Source::ColorBitmap(StrikeWith::BestFit),
                    // Standard scalable outline
                    Source::Outline,
                ])
                // Select a subpixel format
                .format(Format::Alpha)
                // Apply the fractional offset
                .offset(offset)
                // Render the image
                .render(&mut scaler, cache_key.glyph_id)
            });

            if let Some(ref image) = image_opt {
                use swash::scale::image::Content;

                let x = x_int + image.placement.left;
                let y = y_int - image.placement.top;

                match image.content {
                    Content::Mask => {
                        let mut i = 0;
                        for off_y in 0..image.placement.height as i32 {
                            for off_x in 0..image.placement.width as i32 {
                                let color = (image.data[i] as u32) << 24 | base & 0xFFFFFF;
                                f(x + off_x, y + off_y, color);
                                i += 1;
                            }
                        }
                    }
                    Content::Color => {
                        let mut i = 0;
                        for off_y in 0..image.placement.height as i32 {
                            for off_x in 0..image.placement.width as i32 {
                                let color = (image.data[i + 3] as u32) << 24
                                    | (image.data[i] as u32) << 16
                                    | (image.data[i + 1] as u32) << 8
                                    | (image.data[i + 2] as u32);
                                f(x + off_x, y + off_y, color);
                                i += 4;
                            }
                        }
                    }
                    Content::SubpixelMask => {
                        log::warn!("TODO: SubpixelMask");
                    }
                }
            }
        }
    }
}
