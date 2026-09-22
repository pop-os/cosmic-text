mod common;

use common::test_font_system;
use cosmic_text::{
    Attrs, Buffer, CacheKeyFlags, Color, Family, Metrics, Shaping, SwashCache, SwashContent,
};
const MARGIN: i32 = 8;
const BAND_HEIGHT: i32 = 28;

#[test]
fn subpixel_mask_geometry() {
    let mut font_system = test_font_system();
    let mut swash_cache = SwashCache::new();

    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(400.0), None);
    let attrs = Attrs::new().family(Family::Name("Inter"));
    buffer.set_text("Terminal quality 14px", &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, true);

    let glyphs: Vec<_> = buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.to_vec())
        .collect();
    for glyph in glyphs {
        let physical = glyph.physical((0.0, 0.0), 1.0);
        let alpha = swash_cache
            .get_image_uncached(&mut font_system, physical.cache_key)
            .unwrap();
        let mut sub_key = physical.cache_key;
        sub_key.flags |= CacheKeyFlags::SUBPIXEL_RGB;
        let sub = swash_cache
            .get_image_uncached(&mut font_system, sub_key)
            .unwrap();

        // BGR masks must be exact channel swaps of RGB masks, alpha included.
        // This only pins the two orders against each other; the absolute
        // stripe orientation is covered by `subpixel_mask_channel_order`.
        let mut bgr_key = physical.cache_key;
        bgr_key.flags |= CacheKeyFlags::SUBPIXEL_BGR;
        let bgr = swash_cache
            .get_image_uncached(&mut font_system, bgr_key)
            .unwrap();
        assert_eq!(
            (bgr.placement.left, bgr.placement.top),
            (sub.placement.left, sub.placement.top),
            "glyph {:?}",
            glyph.glyph_id
        );
        assert_eq!(
            (bgr.placement.width, bgr.placement.height),
            (sub.placement.width, sub.placement.height),
            "glyph {:?}",
            glyph.glyph_id
        );
        assert_eq!(bgr.data.len(), sub.data.len(), "glyph {:?}", glyph.glyph_id);
        for (rgb_px, bgr_px) in sub.data.chunks_exact(4).zip(bgr.data.chunks_exact(4)) {
            assert_eq!(rgb_px, [bgr_px[2], bgr_px[1], bgr_px[0], bgr_px[3]]);
        }

        let alpha_sum: u32 = alpha.data.iter().map(|&a| u32::from(a)).sum();
        if alpha_sum == 0 {
            continue;
        }

        // Vertical placement must be untouched by the horizontal supersample
        assert_eq!(
            sub.placement.top, alpha.placement.top,
            "glyph {:?}",
            glyph.glyph_id
        );
        assert_eq!(
            sub.placement.height, alpha.placement.height,
            "glyph {:?}",
            glyph.glyph_id
        );

        // Filter padding: about one pixel on each side of the alpha placement
        let (al, aw) = (alpha.placement.left, alpha.placement.width as i32);
        let (sl, sw) = (sub.placement.left, sub.placement.width as i32);
        assert!(
            (al - 2..=al).contains(&sl),
            "glyph {:?}: sub left {} vs alpha left {}",
            glyph.glyph_id,
            sl,
            al
        );
        assert!(
            (al + aw..=al + aw + 2).contains(&(sl + sw)),
            "glyph {:?}: sub right {} vs alpha right {}",
            glyph.glyph_id,
            sl + sw,
            al + aw
        );

        // The FIR taps sum to 256, so total coverage is conserved; the 3x
        // rasterization differs from 1x only by sampling resolution
        let sub_sum: u32 = sub.data.chunks_exact(4).map(|px| u32::from(px[3])).sum();
        let ratio = sub_sum as f64 / alpha_sum as f64;
        assert!(
            (0.9..=1.1).contains(&ratio),
            "glyph {:?}: coverage ratio {}",
            glyph.glyph_id,
            ratio
        );

        // First and last output columns must only hold filter spill, not
        // clipped glyph energy: max A there stays below the filter tail share
        for col in [0, sw - 1] {
            let max_edge = (0..sub.placement.height as i32)
                .map(|row| sub.data[((row * sw + col) as usize) * 4 + 3])
                .max()
                .unwrap();
            assert!(
                max_edge < 128,
                "glyph {:?}: edge column {} has coverage {}",
                glyph.glyph_id,
                col,
                max_edge
            );
        }
    }
}

#[test]
fn subpixel_mask_composite() {
    let mut font_system = test_font_system();
    let mut swash_cache = SwashCache::new();

    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(400.0), None);
    let attrs = Attrs::new().family(Family::Name("Inter"));
    buffer.set_text("Terminal quality 14px", &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, true);

    let line_w = buffer
        .layout_runs()
        .map(|run| run.line_w)
        .fold(0.0f32, f32::max);
    let width = line_w.ceil() as i32 + 2 * MARGIN;
    let height = 2 * BAND_HEIGHT + 2 * MARGIN;

    for (band, subpixel) in [(0, false), (1, true)] {
        let band_top = MARGIN + band * BAND_HEIGHT;
        let runs: Vec<_> = buffer
            .layout_runs()
            .map(|run| (run.line_y, run.glyphs.to_vec()))
            .collect();
        for (line_y, glyphs) in runs {
            for glyph in glyphs {
                let mut physical = glyph.physical((0.0, 0.0), 1.0);
                if subpixel {
                    physical.cache_key.flags |= CacheKeyFlags::SUBPIXEL_RGB;
                }
                let Some(image) =
                    swash_cache.get_image_uncached(&mut font_system, physical.cache_key)
                else {
                    continue;
                };

                if subpixel {
                    assert_eq!(image.content, SwashContent::SubpixelMask);
                    assert_eq!(
                        image.data.len(),
                        (image.placement.width * image.placement.height) as usize * 4
                    );
                } else {
                    assert_eq!(image.content, SwashContent::Mask);
                }

                let origin_x = MARGIN + physical.x + image.placement.left;
                let origin_y = band_top + line_y.round() as i32 + physical.y - image.placement.top;
                for row in 0..image.placement.height as i32 {
                    for col in 0..image.placement.width as i32 {
                        if subpixel {
                            let i = (row * image.placement.width as i32 + col) as usize * 4;
                            let [r, g, b, a]: [u8; 4] = image.data[i..i + 4].try_into().unwrap();
                            assert_eq!(
                                u32::from(a),
                                (u32::from(r) + u32::from(g) + u32::from(b) + 1) / 3
                            );
                        }
                        // Placement must keep every glyph pixel inside the
                        // laid-out box; the subpixel path widens it by the
                        // filter tail and must still not escape.
                        let (x, y) = (origin_x + col, origin_y + row);
                        assert!(
                            (0..width).contains(&x) && (0..height).contains(&y),
                            "glyph {:?} pixel out of bounds: ({x}, {y})",
                            glyph.glyph_id
                        );
                    }
                }
            }
        }
    }
}

/// `with_pixels` has no Color-per-pixel way to carry per-channel coverage, so
/// the `SubpixelMask` arm degrades to the mask's baked-in mean alpha. Pins the
/// geometry and the channel it reads; base alpha handling is deliberately not
/// asserted, since that is an open TODO shared by all three content arms.
#[test]
fn subpixel_mask_with_pixels_emits_mean_alpha() {
    let mut font_system = test_font_system();
    let mut swash_cache = SwashCache::new();

    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(400.0), None);
    let attrs = Attrs::new().family(Family::Name("Inter"));
    buffer.set_text("Terminal quality 14px", &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, true);

    let base = Color::rgb(0x40, 0x80, 0xC0);
    for glyph in buffer.layout_runs().flat_map(|run| run.glyphs.to_vec()) {
        let mut sub_key = glyph.physical((0.0, 0.0), 1.0).cache_key;
        sub_key.flags |= CacheKeyFlags::SUBPIXEL_RGB;

        let image = swash_cache
            .get_image_uncached(&mut font_system, sub_key)
            .unwrap();
        assert_eq!(image.content, SwashContent::SubpixelMask);

        let mut pixels = Vec::new();
        swash_cache.with_pixels(&mut font_system, sub_key, base, |x, y, color| {
            pixels.push((x, y, color))
        });
        assert_eq!(
            pixels.len(),
            (image.placement.width * image.placement.height) as usize
        );

        let x = image.placement.left;
        let y = -image.placement.top;
        for (idx, (px, py, color)) in pixels.iter().enumerate() {
            assert_eq!(
                (*px, *py),
                (
                    x + (idx as i32 % image.placement.width as i32),
                    y + (idx as i32 / image.placement.width as i32)
                )
            );
            // Alpha comes from the mask's A channel, not one of the stripes
            assert_eq!(
                color.0 >> 24,
                u32::from(image.data[idx * 4 + 3]),
                "pixel {idx}"
            );
            assert_eq!(color.0 & 0xFF_FF_FF, base.0 & 0xFF_FF_FF);
        }
    }
}

/// Horizontal centroid of each stored channel, in output-pixel units.
///
/// Returns `None` for masks with no coverage.
fn channel_centroids(image: &cosmic_text::SwashImage) -> Option<[f64; 3]> {
    let width = image.placement.width as usize;
    let mut weighted = [0f64; 3];
    let mut total = [0f64; 3];
    for (i, px) in image.data.chunks_exact(4).enumerate() {
        let x = (i % width) as f64;
        for c in 0..3 {
            weighted[c] += x * f64::from(px[c]);
            total[c] += f64::from(px[c]);
        }
    }
    if total.contains(&0.0) {
        return None;
    }
    Some([0, 1, 2].map(|c| weighted[c] / total[c]))
}

/// The RGB/BGR swap check passes even if both orders are reversed, which is
/// zeno's `Format::Subpixel` bug. Pin the absolute orientation instead:
/// channel `k` samples subpixel column `3p + k`, so each channel's centroid
/// sits 1/3 pixel left of the next for RGB stripe order, and right for BGR.
#[test]
fn subpixel_mask_channel_order() {
    let mut font_system = test_font_system();
    let mut swash_cache = SwashCache::new();

    let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
    buffer.set_size(Some(600.0), None);
    let attrs = Attrs::new().family(Family::Name("Inter"));
    buffer.set_text(
        "Terminal quality WAVE ilj 0123 MgQ",
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, true);

    // Wide enough to admit rasterization noise, tight enough to reject a
    // grayscale collapse (0), a reversed order (opposite sign), and an
    // off-by-one subpixel column (0 or 2/3).
    let expected = 1.0 / 3.0;
    let tolerance = 1.0 / 8.0;

    let mut checked = 0;
    for glyph in buffer.layout_runs().flat_map(|run| run.glyphs.to_vec()) {
        for (flag, sign) in [
            (CacheKeyFlags::SUBPIXEL_RGB, 1.0),
            (CacheKeyFlags::SUBPIXEL_BGR, -1.0),
        ] {
            let mut key = glyph.physical((0.0, 0.0), 1.0).cache_key;
            key.flags |= flag;
            let image = swash_cache
                .get_image_uncached(&mut font_system, key)
                .unwrap();
            let Some(centroid) = channel_centroids(&image) else {
                continue;
            };
            for (a, b) in [(0, 1), (1, 2)] {
                let delta = sign * (centroid[a] - centroid[b]);
                assert!(
                    (delta - expected).abs() < tolerance,
                    "glyph {:?} {flag:?}: centroid[{a}] - centroid[{b}] = {:+.4}, \
                     expected {:+.4} for this stripe order",
                    glyph.glyph_id,
                    sign * delta,
                    sign * expected
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no glyphs with coverage were checked");
}
