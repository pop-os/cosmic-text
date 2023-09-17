// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "ab_glyph_image")]
use alloc::borrow::Cow;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use ab_glyph::Font as AbGlyphFont;
use ab_glyph::FontRef;
use self_cell::self_cell;
#[cfg(feature = "ab_glyph_image")]
use zune_png::zune_core::colorspace::ColorSpace;

use crate::{CacheKey, Color, Draw, Font, FontSystem, LayoutRun};

type BuildHasher = core::hash::BuildHasherDefault<rustc_hash::FxHasher>;

#[cfg(feature = "std")]
type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasher>;
#[cfg(not(feature = "std"))]
type HashMap<K, V> = hashbrown::HashMap<K, V, BuildHasher>;

self_cell!(
    struct OwnedFont {
        owner: Arc<Font>,

        #[covariant]
        dependent: FontRef,
    }

    impl {Debug}
);

#[derive(Debug)]
enum RenderGlyph {
    Outline {
        bounds: ab_glyph::Rect,
        coverage: Vec<u8>,
    },

    #[cfg(feature = "ab_glyph_image")]
    Image {
        width: usize,
        height: usize,
        offset_x: i32,
        offset_y: i32,
        bitmap: Vec<Color>,
    },
}

#[derive(Debug, Default)]
pub struct AbGlyphDraw {
    font_cache: HashMap<fontdb::ID, Option<OwnedFont>>,
    glyph_cache: HashMap<CacheKey, Option<RenderGlyph>>,
}

impl AbGlyphDraw {
    pub fn new() -> Self {
        Self::default()
    }

    fn render_glyph(
        &mut self,
        font_system: &mut FontSystem,
        cache_key: &CacheKey,
    ) -> Option<RenderGlyph> {
        let font_size = f32::from_bits(cache_key.font_size_bits);

        let font = self
            .font_cache
            .entry(cache_key.font_id)
            .or_insert_with(|| {
                let font = font_system.get_font(cache_key.font_id)?;
                OwnedFont::try_new(font, |font| ab_glyph::FontRef::try_from_slice(font.data()))
                    .ok()
                    .or_else(|| {
                        let face = font_system.db().face(cache_key.font_id)?;
                        log::warn!(
                            "failed to load font '{}' for ab_glyph",
                            face.post_script_name
                        );
                        None
                    })
            })
            .as_ref()?
            .borrow_dependent();

        let glyph_id = ab_glyph::GlyphId(cache_key.glyph_id);

        // ab_glyph treat units of font height as 1 em instead of conventional units_per_em,
        // and causing glyphs being rendered smaller. We re-scale it back to align with
        // layouting result, see <https://github.com/alexheretic/ab-glyph/issues/15>
        let rescale_factor = font
            .units_per_em()
            .map(|units_per_em| font.height_unscaled() / units_per_em)
            .unwrap_or(1.0);
        let glyph = glyph_id.with_scale_and_position(
            font_size * rescale_factor,
            ab_glyph::point(cache_key.x_bin.as_float(), cache_key.y_bin.as_float()),
        );

        if let Some(outlined_glyph) = font.outline_glyph(glyph) {
            let bounds = outlined_glyph.px_bounds();
            let width = bounds.width() as usize;
            let height = bounds.height() as usize;
            let mut coverage = vec![0u8; width * height];
            outlined_glyph.draw(|x, y, c| {
                let id = x as usize + y as usize * width;
                coverage[id] = (c * 255.0) as _;
            });
            return Some(RenderGlyph::Outline { bounds, coverage });
        }

        #[cfg(feature = "ab_glyph_image")]
        if let Some(image) = font.glyph_raster_image2(glyph_id, font_size as _) {
            return convert_glyph_image(image, font_size);
        };

        None
    }
}

#[cfg(feature = "ab_glyph_image")]
fn convert_glyph_image(image: ab_glyph::v2::GlyphImage, font_size: f32) -> Option<RenderGlyph> {
    match image.format {
        ab_glyph::GlyphImageFormat::Png => {
            use zune_png::zune_core::bit_depth::BitDepth;
            let mut decoder = zune_png::PngDecoder::new(image.data);
            let res = match decoder.decode() {
                Ok(res) => res,
                Err(e) => {
                    log::error!("failed to decode PNG image: {:?}", e);
                    return None;
                }
            };

            let info = decoder.get_info()?;
            let color_space = decoder.get_colorspace()?;

            let mut data_u8;
            let data_u16;
            let data = match decoder.get_depth()? {
                BitDepth::Eight => {
                    data_u8 = res.u8()?;
                    convert_u8_to_rgba8(&mut data_u8, color_space)?
                }
                BitDepth::Sixteen => {
                    data_u16 = res.u16()?;
                    Cow::Owned(convert_u16_to_rgba8(&data_u16, color_space)?)
                }
                _ => {
                    return None;
                }
            };

            let scale = font_size / image.pixels_per_em as f32;
            let offset_x = (-image.origin.x * scale) as i32;
            let offset_y = (-(image.origin.y + info.height as f32) * scale) as i32;

            let (data, width, height) = resize_rgba8(&data, info.width, info.height, scale)?;

            Some(RenderGlyph::Image {
                width,
                height,
                offset_x,
                offset_y,
                bitmap: rgba8_to_color_bitmap(&data),
            })
        }
        _ => {
            // XXX: current ab_glyph api does not provides width and height info for glyph
            // image so there is no way to indexing the bitmap data, though we can decode
            // those info from PNG data header
            log::warn!("glyph image format {:?} not supported yet", image.format);
            None
        }
    }
}

impl Draw for AbGlyphDraw {
    fn draw_line<F>(
        &mut self,
        font_system: &mut FontSystem,
        run: &LayoutRun,
        color: Color,
        f: &mut F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        for glyph in run.glyphs.iter() {
            let physical_glyph = glyph.physical((0., 0.), 1.0);

            let glyph_color = glyph.color_opt.unwrap_or(color);

            // move out glyph_cache to allow mutable reference to self in closure
            let mut glyph_cache = core::mem::take(&mut self.glyph_cache);
            let render_glyph = glyph_cache
                .entry(physical_glyph.cache_key)
                .or_insert_with(|| self.render_glyph(font_system, &physical_glyph.cache_key))
                .as_ref();

            if let Some(render_glyph) = render_glyph {
                match render_glyph {
                    RenderGlyph::Outline { bounds, coverage } => {
                        let width = bounds.width() as usize;
                        let height = bounds.height() as usize;

                        for y in 0..height {
                            for x in 0..width {
                                let color = color_blend_alpha(glyph_color, coverage[x + y * width]);
                                let x = x as i32 + physical_glyph.x + (bounds.min.x) as i32;
                                let y = y as i32
                                    + physical_glyph.y
                                    + (run.line_y + bounds.min.y) as i32;

                                f(x, y, 1, 1, color);
                            }
                        }
                    }

                    #[cfg(feature = "ab_glyph_image")]
                    RenderGlyph::Image {
                        width,
                        height,
                        offset_x,
                        offset_y,
                        bitmap,
                    } => {
                        for y in 0..*height {
                            for x in 0..*width {
                                let color = bitmap[x + y * width];
                                let x = x as i32 + physical_glyph.x + offset_x;
                                let y = y as i32 + physical_glyph.y + run.line_y as i32 + offset_y;

                                f(x, y, 1, 1, color);
                            }
                        }
                    }
                }
            };
            // move back glyph_cache
            self.glyph_cache = glyph_cache;
        }
    }
}

// Copied from tiny-skia
#[inline]
fn premultiply_u8(c: u8, a: u8) -> u8 {
    let prod = u32::from(c) * u32::from(a) + 128;
    ((prod + (prod >> 8)) >> 8) as u8
}

#[inline]
fn color_blend_alpha(c: Color, a: u8) -> Color {
    let a = premultiply_u8(c.a(), a);
    Color(((a as u32) << 24) | c.0 & 0xFF_FF_FF)
}

#[cfg(feature = "ab_glyph_image")]
fn convert_u8_to_rgba8(data: &mut [u8], color_space: ColorSpace) -> Option<Cow<[rgb::RGBA8]>> {
    use rgb::FromSlice;
    let data = match color_space {
        ColorSpace::RGBA => Cow::Borrowed(data.as_rgba()),
        ColorSpace::BGRA => {
            data.as_bgra_mut()
                .iter_mut()
                .for_each(|c| core::mem::swap(&mut c.b, &mut c.r));
            Cow::Borrowed(data.as_rgba())
        }
        ColorSpace::RGB => {
            let data = data
                .as_rgb()
                .iter()
                .map(|c| rgb::RGBA8::new(c.r, c.g, c.b, 255))
                .collect();
            Cow::Owned(data)
        }
        ColorSpace::BGR => {
            let data = data
                .as_bgr()
                .iter()
                .map(|c| rgb::RGBA8::new(c.r, c.g, c.b, 255))
                .collect();
            Cow::Owned(data)
        }
        ColorSpace::Luma => {
            let data = data
                .as_gray()
                .iter()
                .map(|c| rgb::RGBA8::new(c.0, c.0, c.0, 255))
                .collect();
            Cow::Owned(data)
        }
        ColorSpace::LumaA => {
            let data = data
                .as_gray_alpha()
                .iter()
                .map(|c| rgb::RGBA8::new(c.0, c.0, c.0, c.1))
                .collect();
            Cow::Owned(data)
        }
        _ => {
            // PNG does not support these color spaces, it's fine to not implement support for them
            log::warn!(
                "glyph image color space {:?} not supported yet",
                color_space
            );
            return None;
        }
    };
    Some(data)
}

#[cfg(feature = "ab_glyph_image")]
fn convert_u16_to_rgba8(data: &[u16], color_space: ColorSpace) -> Option<Vec<rgb::RGBA8>> {
    use rgb::FromSlice;

    #[inline(always)]
    fn cvt(color: u16) -> u8 {
        ((color + 0b0_1000_0000) >> 8) as u8
    }

    let data: Vec<rgb::RGBA8> = match color_space {
        ColorSpace::RGBA => data
            .as_rgba()
            .iter()
            .map(|c| rgb::RGBA8::new(cvt(c.r), cvt(c.g), cvt(c.b), cvt(c.a)))
            .collect(),
        ColorSpace::BGRA => data
            .as_bgra()
            .iter()
            .map(|c| rgb::RGBA8::new(cvt(c.r), cvt(c.g), cvt(c.b), cvt(c.a)))
            .collect(),
        ColorSpace::RGB => data
            .as_rgb()
            .iter()
            .map(|c| rgb::RGBA8::new(cvt(c.r), cvt(c.g), cvt(c.b), 0xff))
            .collect(),
        ColorSpace::BGR => data
            .as_bgr()
            .iter()
            .map(|c| rgb::RGBA8::new(cvt(c.r), cvt(c.g), cvt(c.b), 0xff))
            .collect(),
        ColorSpace::Luma => data
            .as_gray()
            .iter()
            .map(|c| {
                let l = cvt(c.0);
                rgb::RGBA8::new(l, l, l, 0xff)
            })
            .collect(),
        ColorSpace::LumaA => data
            .as_gray_alpha()
            .iter()
            .map(|c| {
                let l = cvt(c.0);
                rgb::RGBA8::new(l, l, l, cvt(c.1))
            })
            .collect(),
        _ => {
            // PNG does not support these color spaces, it's fine to not implement support for them
            log::warn!(
                "glyph image color space {:?} not supported yet",
                color_space
            );
            return None;
        }
    };
    Some(data)
}

#[cfg(feature = "ab_glyph_image")]
fn rgba8_to_color_bitmap(data: &[rgb::RGBA8]) -> Vec<Color> {
    data.iter()
        .map(|c| Color::rgba(c.r, c.g, c.b, c.a))
        .collect()
}

#[cfg(feature = "ab_glyph_image")]
fn resize_rgba8(
    data: &[rgb::RGBA8],
    width: usize,
    height: usize,
    scale: f32,
) -> Option<(Cow<[rgb::RGBA8]>, usize, usize)> {
    if scale == 1.0 {
        return Some((Cow::Borrowed(data), width, height));
    }

    let dst_width = (width as f32 * scale) as usize;
    let dst_height = (height as f32 * scale) as usize;

    let mut resizer = resize::new(
        width,
        height,
        dst_width,
        dst_height,
        resize::Pixel::RGBA8P,
        resize::Type::Mitchell,
    )
    .map_err(|e| {
        log::error!("failed to create resizer: {}", e);
        e
    })
    .ok()?;

    let mut dst = vec![rgb::RGBA8::default(); dst_width * dst_height];
    resizer
        .resize(data, &mut dst)
        .map_err(|e| {
            log::error!("failed to resize: {}", e);
            e
        })
        .ok()?;
    Some((Cow::Owned(dst), dst_width, dst_height))
}
