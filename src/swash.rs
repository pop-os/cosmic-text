// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use swash::scale::{ScaleContext, image::Content};
use swash::scale::{Render, Source, StrikeWith};
use swash::zeno::{Format, Vector};

use crate::{CacheKey, FontMatches};

pub use swash::scale::image::Image as SwashImage;

pub struct SwashCache {
    context: ScaleContext,
    pub image_cache: HashMap<CacheKey, Option<SwashImage>>,
}

impl SwashCache {
    /// Create a new swash cache
    pub fn new() -> Self {
        Self {
            context: ScaleContext::new(),
            image_cache: HashMap::new()
        }
    }

    /// Create a swash Image from a cache key, caching results
    pub fn get_image(&mut self, matches: &FontMatches<'_>, cache_key: CacheKey) -> &Option<SwashImage> {
        self.image_cache.entry(cache_key).or_insert_with(|| {
            let font = match matches.get_font(&cache_key.font_id) {
                Some(some) => some,
                None => {
                    log::warn!("did not find font {:?}", cache_key.font_id);
                    return None;
                },
            };

            // Build the scaler
            let mut scaler = self.context
                .builder(font.as_swash())
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
        })
    }

    /// Enumerate pixels in an Image, use `with_image` for better performance
    pub fn with_pixels<F: FnMut(i32, i32, u32)>(
        &mut self,
        matches: &FontMatches<'_>,
        cache_key: CacheKey,
        base: u32,
        mut f: F
    ) {
        if let Some(image) = self.get_image(matches, cache_key) {
            let x = image.placement.left;
            let y = -image.placement.top;

            match image.content {
                Content::Mask => {
                    let mut i = 0;
                    for off_y in 0..image.placement.height as i32 {
                        for off_x in 0..image.placement.width as i32 {
                            //TODO: blend base alpha?
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
                            //TODO: blend base alpha?
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
