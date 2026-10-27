// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec};
#[cfg(feature = "no_std")]
use core_maths::CoreFloat;

use core::fmt;
use swash::scale::{image::Content, ScaleContext};
use swash::scale::{Render, Source, StrikeWith};
use swash::zeno::{Format, Vector};

use crate::{CacheKey, CacheKeyFlags, Color, FontSystem, HashMap};

pub use swash::scale::image::{Content as SwashContent, Image as SwashImage};
pub use swash::zeno::{Angle, Command, Placement, Transform};

fn swash_image(
    font_system: &mut FontSystem,
    context: &mut ScaleContext,
    cache_key: CacheKey,
) -> Option<SwashImage> {
    let Some(font) = font_system.get_font(cache_key.font_id, cache_key.font_weight) else {
        log::warn!("did not find font {:?}", cache_key.font_id);
        return None;
    };

    let variable_width = font
        .as_swash()
        .variations()
        .find_by_tag(swash::Tag::from_be_bytes(*b"wght"));

    // Build the scaler
    let mut scaler = context
        .builder(font.as_swash())
        .size(f32::from_bits(cache_key.font_size_bits))
        .hint(!cache_key.flags.contains(CacheKeyFlags::DISABLE_HINTING));
    if let Some(variation) = variable_width {
        scaler = scaler.normalized_coords(font.as_swash().variations().normalized_coords([(
            swash::Tag::from_be_bytes(*b"wght"),
            f32::from(cache_key.font_weight.0).clamp(variation.min_value(), variation.max_value()),
        )]));
    }
    let mut scaler = scaler.build();

    // Compute the fractional offset-- you'll likely want to quantize this
    // in a real renderer
    let offset = if cache_key.flags.contains(CacheKeyFlags::PIXEL_FONT) {
        Vector::new(
            cache_key.x_bin.as_float().round(),
            cache_key.y_bin.as_float().round(),
        )
    } else {
        Vector::new(cache_key.x_bin.as_float(), cache_key.y_bin.as_float())
    };

    let fake_italic = if cache_key.flags.contains(CacheKeyFlags::FAKE_ITALIC) {
        Some(Transform::skew(
            Angle::from_degrees(14.0),
            Angle::from_degrees(0.0),
        ))
    } else {
        None
    };

    if cache_key
        .flags
        .intersects(CacheKeyFlags::SUBPIXEL_RGB | CacheKeyFlags::SUBPIXEL_BGR)
    {
        // Color sources keep the standard path; try them first to preserve
        // the source priority order below.
        if let Some(image) = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
        ])
        .format(Format::Alpha)
        .offset(offset)
        .transform(fake_italic)
        .render(&mut scaler, cache_key.glyph_id)
        {
            return Some(image);
        }

        // Scalable outline: rasterize at 3x horizontal resolution. Hinting is
        // vertical-only grid fitting, so post-hint horizontal scaling matches
        // FreeType's LCD mechanism. Render::offset is applied after the
        // transform, in device space, so the fractional x offset scales by 3.
        let transform = match fake_italic {
            Some(skew) => skew.then_scale(3.0, 1.0),
            None => Transform::scale(3.0, 1.0),
        };
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(offset.x * 3.0, offset.y))
            .transform(Some(transform))
            .render(&mut scaler, cache_key.glyph_id)?;
        let bgr = cache_key.flags.contains(CacheKeyFlags::SUBPIXEL_BGR);
        return Some(lcd_filter_downsample(&image, bgr));
    }

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
    .transform(fake_italic)
    // Render the image
    .render(&mut scaler, cache_key.glyph_id)
}

/// `FreeType`'s default LCD filter `lcddefault`, applied on subpixel columns
const LCD_FILTER: [u32; 5] = [8, 77, 86, 77, 8];

/// Downsample a 3x-horizontal alpha mask into an RGB-per-pixel subpixel mask.
///
/// Each output pixel takes its R, G, B coverage from three adjacent subpixel
/// columns, each convolved with [`LCD_FILTER`] along x. The filter spreads
/// energy two subpixel columns past each edge, so the output placement is
/// widened to cover every column holding energy. Output data is RGBA, with
/// A = (R + G + B) / 3 rounded. Channel order follows the physical stripe:
/// little-endian RGBA bytes hold R first for RGB stripes (`bgr == false`),
/// B first for BGR stripes (`bgr == true`).
fn lcd_filter_downsample(image: &SwashImage, bgr: bool) -> SwashImage {
    let mut out = SwashImage::new();
    out.source = image.source;
    out.content = SwashContent::SubpixelMask;

    let w3 = image.placement.width as i32;
    let height = image.placement.height;
    if w3 == 0 || height == 0 {
        // Nothing was rasterized. Leave the default empty placement: the
        // source extent is still in subpixel columns, and reporting it here
        // would hand out a `SubpixelMask` placement in the wrong unit.
        return out;
    }

    // The supersample is horizontal only, so vertical placement carries over
    out.placement.top = image.placement.top;
    out.placement.height = height;

    // Nonzero filtered subpixel columns span [left3 - 2, left3 + w3 + 1].
    // Output pixel p covers subpixel columns 3p .. 3p + 2.
    let left3 = image.placement.left;
    let left = (left3 - 2).div_euclid(3);
    let width = ((left3 + w3 + 1).div_euclid(3) - left + 1) as u32;

    out.placement.left = left;
    out.placement.width = width;
    out.data.resize((width * height) as usize * 4, 0);

    // Each row is copied into a zero-padded buffer covering columns
    // [3*left - 2, 3*(left + width) + 4], which contains every filter tap for
    // every output channel. The padding is only ever read, so the allocation
    // below zeroes it once for the whole glyph and each row overwrites the
    // same interior span.
    let span_start = 3 * left - 2;
    let stride = (3 * width as i32 + 7) as usize;
    let mut padded = vec![0u8; stride];
    let copy_at = (left3 - span_start) as usize;

    for (row_out, row_in) in out
        .data
        .chunks_exact_mut(width as usize * 4)
        .zip(image.data.chunks_exact(w3 as usize))
    {
        padded[copy_at..copy_at + w3 as usize].copy_from_slice(row_in);

        // Seven consecutive columns hold every tap of all three channels, so
        // each output pixel reads one window at a constant stride of three.
        // Indexing a fixed-size array with constants is checked at compile
        // time, which keeps the convolution free of bounds checks.
        let windows = padded.windows(7).step_by(3);
        for (px, window) in row_out.chunks_exact_mut(4).zip(windows) {
            let Some(w) = window.first_chunk::<7>() else {
                break;
            };
            // Coverage of the pixel's three subpixel columns, left to right
            let cov = [
                lcd_tap(&[w[0], w[1], w[2], w[3], w[4]]),
                lcd_tap(&[w[1], w[2], w[3], w[4], w[5]]),
                lcd_tap(&[w[2], w[3], w[4], w[5], w[6]]),
            ];
            let sum = u32::from(cov[0]) + u32::from(cov[1]) + u32::from(cov[2]);
            // Store in physical stripe order; alpha is the rounded mean and
            // is therefore order-independent.
            let [c0, c1, c2] = cov;
            px.copy_from_slice(&if bgr {
                [c2, c1, c0, ((sum + 1) / 3) as u8]
            } else {
                [c0, c1, c2, ((sum + 1) / 3) as u8]
            });
        }
    }
    out
}

/// Convolve one five-column window with [`LCD_FILTER`], rounding to 8-bit
/// coverage. The taps sum to 256, so a fully covered window yields 255.
#[inline(always)]
fn lcd_tap(window: &[u8; 5]) -> u8 {
    let mut acc = 128;
    for (coverage, weight) in window.iter().zip(LCD_FILTER) {
        acc += u32::from(*coverage) * weight;
    }
    (acc >> 8) as u8
}

fn swash_outline_commands(
    font_system: &mut FontSystem,
    context: &mut ScaleContext,
    cache_key: CacheKey,
) -> Option<Box<[swash::zeno::Command]>> {
    use swash::zeno::PathData as _;

    let Some(font) = font_system.get_font(cache_key.font_id, cache_key.font_weight) else {
        log::warn!("did not find font {:?}", cache_key.font_id);
        return None;
    };

    let variable_width = font
        .as_swash()
        .variations()
        .find_by_tag(swash::Tag::from_be_bytes(*b"wght"));

    // Build the scaler
    let mut scaler = context
        .builder(font.as_swash())
        .size(f32::from_bits(cache_key.font_size_bits))
        .hint(!cache_key.flags.contains(CacheKeyFlags::DISABLE_HINTING));
    if let Some(variation) = variable_width {
        scaler = scaler.normalized_coords(font.as_swash().variations().normalized_coords([(
            swash::Tag::from_be_bytes(*b"wght"),
            f32::from(cache_key.font_weight.0).clamp(variation.min_value(), variation.max_value()),
        )]));
    }
    let mut scaler = scaler.build();

    // Scale the outline
    let mut outline = scaler
        .scale_outline(cache_key.glyph_id)
        .or_else(|| scaler.scale_color_outline(cache_key.glyph_id))?;

    if cache_key.flags.contains(CacheKeyFlags::FAKE_ITALIC) {
        outline.transform(&Transform::skew(
            Angle::from_degrees(14.0),
            Angle::from_degrees(0.0),
        ));
    }

    // Get the path information of the outline
    let path = outline.path();

    // Return the commands
    Some(path.commands().collect())
}

/// Cache for rasterizing with the swash scaler
pub struct SwashCache {
    context: ScaleContext,
    pub image_cache: HashMap<CacheKey, Option<SwashImage>>,
    pub outline_command_cache: HashMap<CacheKey, Option<Box<[swash::zeno::Command]>>>,
}

impl fmt::Debug for SwashCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("SwashCache { .. }")
    }
}

impl SwashCache {
    /// Create a new swash cache
    pub fn new() -> Self {
        Self {
            context: ScaleContext::new(),
            image_cache: HashMap::default(),
            outline_command_cache: HashMap::default(),
        }
    }

    /// Create a swash Image from a cache key, without caching results
    pub fn get_image_uncached(
        &mut self,
        font_system: &mut FontSystem,
        cache_key: CacheKey,
    ) -> Option<SwashImage> {
        swash_image(font_system, &mut self.context, cache_key)
    }

    /// Create a swash Image from a cache key, caching results
    pub fn get_image(
        &mut self,
        font_system: &mut FontSystem,
        cache_key: CacheKey,
    ) -> &Option<SwashImage> {
        self.image_cache
            .entry(cache_key)
            .or_insert_with(|| swash_image(font_system, &mut self.context, cache_key))
    }

    /// Creates outline commands
    pub fn get_outline_commands(
        &mut self,
        font_system: &mut FontSystem,
        cache_key: CacheKey,
    ) -> Option<&[swash::zeno::Command]> {
        self.outline_command_cache
            .entry(cache_key)
            .or_insert_with(|| swash_outline_commands(font_system, &mut self.context, cache_key))
            .as_deref()
    }

    /// Creates outline commands, without caching results
    pub fn get_outline_commands_uncached(
        &mut self,
        font_system: &mut FontSystem,
        cache_key: CacheKey,
    ) -> Option<Box<[swash::zeno::Command]>> {
        swash_outline_commands(font_system, &mut self.context, cache_key)
    }

    /// Enumerate pixels in an Image, use `with_image` for better performance
    pub fn with_pixels<F: FnMut(i32, i32, Color)>(
        &mut self,
        font_system: &mut FontSystem,
        cache_key: CacheKey,
        base: Color,
        mut f: F,
    ) {
        if let Some(image) = self.get_image(font_system, cache_key) {
            let x = image.placement.left;
            let y = -image.placement.top;

            match image.content {
                Content::Mask => {
                    let mut i = 0;
                    for off_y in 0..image.placement.height as i32 {
                        for off_x in 0..image.placement.width as i32 {
                            //TODO: blend base alpha?
                            f(
                                x + off_x,
                                y + off_y,
                                Color((u32::from(image.data[i]) << 24) | base.0 & 0xFF_FF_FF),
                            );
                            i += 1;
                        }
                    }
                }
                Content::Color => {
                    let mut i = 0;
                    for off_y in 0..image.placement.height as i32 {
                        for off_x in 0..image.placement.width as i32 {
                            //TODO: blend base alpha?
                            f(
                                x + off_x,
                                y + off_y,
                                Color::rgba(
                                    image.data[i],
                                    image.data[i + 1],
                                    image.data[i + 2],
                                    image.data[i + 3],
                                ),
                            );
                            i += 4;
                        }
                    }
                }
                Content::SubpixelMask => {
                    // Per-channel coverage cannot be expressed through the
                    // single Color-per-pixel API; degrade to the baked-in
                    // mean-coverage alpha, matching Content::Mask output.
                    let mut i = 3;
                    for off_y in 0..image.placement.height as i32 {
                        for off_x in 0..image.placement.width as i32 {
                            //TODO: blend base alpha?
                            f(
                                x + off_x,
                                y + off_y,
                                Color((u32::from(image.data[i]) << 24) | base.0 & 0xFF_FF_FF),
                            );
                            i += 4;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use swash::{FontRef, Setting, Tag};

    // variations() resizes context.coords in place (stale values persist),
    // whereas using normalized_coords() clears and replaces them.
    #[test]
    fn no_coord_leakage_across_fonts() {
        let [Ok(sfns), Ok(sfns_italic)] = [
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/SFNSItalic.ttf",
        ]
        .map(std::fs::read) else {
            return;
        };
        let regular = FontRef::from_index(&sfns, 0).unwrap();
        let italic = FontRef::from_index(&sfns_italic, 0).unwrap();
        let wght = Tag::from_be_bytes(*b"wght");

        let render = |ctx: &mut ScaleContext, font: FontRef, weight: f32, use_normalized| {
            let mut b = ctx.builder(font).size(16.0).hint(true);
            if use_normalized {
                b = b.normalized_coords(font.variations().normalized_coords([(wght, weight)]));
            } else {
                b = b.variations(std::iter::once(Setting {
                    tag: wght,
                    value: weight,
                }));
            }
            Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .render(&mut b.build(), 36)
        };

        // reference: regular@400 with no prior context
        let mut ctx = ScaleContext::new();
        let reference = render(&mut ctx, regular, 400.0, false).map(|i| i.data);

        // variations(): pollute ctx with italic@700, then render regular@400
        let mut ctx = ScaleContext::new();
        render(&mut ctx, italic, 700.0, false);
        let not_normalized = render(&mut ctx, regular, 400.0, false).map(|i| i.data);

        // normalized_coords(): same sequence
        let mut ctx = ScaleContext::new();
        render(&mut ctx, italic, 700.0, true);
        let normalized = render(&mut ctx, regular, 400.0, true).map(|i| i.data);

        assert_ne!(not_normalized, reference, "variations leak across fonts");
        assert_eq!(
            normalized, reference,
            "normalized_coords match clean render"
        );
    }
}
