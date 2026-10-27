//! Compares glyph rasterization through `SwashCache` between the standard
//! alpha mask path and the opt-in LCD subpixel mask path
//! (`CacheKeyFlags::SUBPIXEL_RGB`).
//!
//! Two regimes are measured per font size:
//! - "uncached": every call goes through the scaler + (for subpixel) the LCD
//!   FIR downsample. This is the cost paid on cache misses, e.g. when scrolling
//!   new content into view.
//! - "cached": repeated lookups against a pre-warmed image cache. Subpixel
//!   images hold roughly 3x wider x RGBA vs single-byte alpha data, so cache
//!   footprint and lookup/blit costs can diverge.

use cosmic_text::{
    Attrs, Buffer, CacheKey, CacheKeyFlags, FontSystem, Metrics, Shaping, SwashCache,
};
use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashSet;
use std::hint::black_box;

/// Shape `text` and collect the deduplicated glyph cache keys.
fn collect_cache_keys(text: &str, font_size: f32) -> (FontSystem, Vec<CacheKey>) {
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(font_size, font_size * 1.4));
    buffer.set_size(Some(400.0), None);
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, true);

    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let key = glyph.physical((0.0, 0.0), 1.0).cache_key;
            if seen.insert(key) {
                keys.push(key);
            }
        }
    }
    (font_system, keys)
}

fn bench_glyph_rasterization(c: &mut Criterion) {
    let text = include_str!("../sample/hello.txt");

    for (label, size) in [("14px", 14.0), ("32px", 32.0)] {
        let (mut font_system, keys) = collect_cache_keys(text, size);
        let glyph_count = keys.len();

        let mut group = c.benchmark_group(format!("glyph rasterization/{label}"));
        group.throughput(criterion::Throughput::Elements(glyph_count as u64));

        // Cold path: full rasterization on every call
        group.bench_function("uncached/alpha", |b| {
            let mut swash_cache = SwashCache::new();
            b.iter(|| {
                for key in &keys {
                    black_box(swash_cache.get_image_uncached(&mut font_system, *key));
                }
            });
        });

        group.bench_function("uncached/subpixel", |b| {
            let mut swash_cache = SwashCache::new();
            b.iter(|| {
                for key in &keys {
                    let mut key = *key;
                    key.flags |= CacheKeyFlags::SUBPIXEL_RGB;
                    black_box(swash_cache.get_image_uncached(&mut font_system, key));
                }
            });
        });

        // Warm path: pre-warm both caches, then measure lookups only
        group.bench_function("cached/alpha", |b| {
            let mut swash_cache = SwashCache::new();
            for key in &keys {
                swash_cache.get_image(&mut font_system, *key);
            }
            b.iter(|| {
                for key in &keys {
                    black_box(swash_cache.get_image(&mut font_system, *key));
                }
            });
        });

        group.bench_function("cached/subpixel", |b| {
            let mut swash_cache = SwashCache::new();
            for key in &keys {
                let mut key = *key;
                key.flags |= CacheKeyFlags::SUBPIXEL_RGB;
                swash_cache.get_image(&mut font_system, key);
            }
            b.iter(|| {
                for key in &keys {
                    let mut key = *key;
                    key.flags |= CacheKeyFlags::SUBPIXEL_RGB;
                    black_box(swash_cache.get_image(&mut font_system, key));
                }
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_glyph_rasterization);
criterion_main!(benches);
