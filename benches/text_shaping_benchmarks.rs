use cosmic_text as ct;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

struct CtBencher {
    font_system: ct::FontSystem,
}

impl CtBencher {
    fn new() -> Self {
        let font_system = ct::FontSystem::new();
        Self { font_system }
    }

    fn shape_and_layout_text(&mut self, text: &str, font_size: f32, line_height: f32, width: f32) {
        let mut buffer = ct::Buffer::new(
            &mut self.font_system,
            ct::Metrics::new(font_size, line_height),
        );
        buffer.set_size(&mut self.font_system, Some(width), None);
        buffer.set_text(
            &mut self.font_system,
            black_box(&text),
            &ct::Attrs::new(),
            ct::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);
        black_box(&mut buffer);
    }
}

struct ParleyBencher {
    font_ctx: parley::FontContext,
    layout_ctx: parley::LayoutContext,
}

impl ParleyBencher {
    fn new() -> Self {
        let font_ctx = parley::FontContext::new();
        let layout_ctx = parley::LayoutContext::new();
        Self {
            font_ctx,
            layout_ctx,
        }
    }

    fn shape_and_layout_text(&mut self, text: &str, font_size: f32, line_height: f32, width: f32) {
        let mut builder =
            self.layout_ctx
                .ranged_builder(&mut self.font_ctx, black_box(&text), 1.0, false);
        builder.push_default(parley::StyleProperty::FontSize(font_size));
        builder.push_default(parley::LineHeight::Absolute(line_height));
        let mut layout = builder.build(black_box(&text));
        layout.break_all_lines(Some(width));
        black_box(&mut layout);
    }
}

fn bench_both(c: &mut Criterion, name: &str, text: &str) {
    let mut group = c.benchmark_group(name);

    let mut ct_bencher = CtBencher::new();
    group.bench_function("cosmic_text", |b| {
        b.iter(|| {
            ct_bencher.shape_and_layout_text(&text, 14.0, 20.0, 500.0);
        });
    });

    let mut parley_bencher = ParleyBencher::new();
    group.bench_function("parley", |b| {
        b.iter(|| {
            parley_bencher.shape_and_layout_text(&text, 14.0, 20.0, 500.0);
        });
    });
}

fn bench_ascii_fast_path(c: &mut Criterion) {
    let ascii_text = "Pure ASCII text for BidiParagraphs optimization testing.\n".repeat(50);
    bench_both(c, "ShapeLine/ASCII Fast Path", &ascii_text);
}

fn bench_bidi_processing(c: &mut Criterion) {
    let bidi_text = "Mixed English and العربية النص العربي text for BiDi testing.\nThis tests adjust_levels and combined BiDi optimizations.\n".repeat(30);
    bench_both(c, "ShapeLine/BiDi Processing", &bidi_text);
}

fn bench_lang_mixed(c: &mut Criterion) {
    let bidi_text = include_str!("../sample/hello.txt");
    bench_both(c, "ShapeLine/Mixed-Language Text", &bidi_text);
}

fn bench_layout_heavy(c: &mut Criterion) {
    let layout_text = "This is a very long line that will wrap multiple times and stress the reorder optimization through intensive layout processing with comprehensive buffer reuse testing. ".repeat(30);
    bench_both(c, "ShapeLine/Layout Heavy", &layout_text);
}

fn bench_combined_stress(c: &mut Criterion) {
    let stress_text = format!("{}\n{}\n{}\n{}\n",
        "ASCII line for BidiParagraphs optimization. ".repeat(15),
        "Mixed English + العربية for BiDi optimizations. ".repeat(12),
        "Very long wrapping line that will trigger reorder optimizations multiple times through intensive layout processing. ".repeat(8),
        "Cache key generation line for ShapeRunKey optimization testing. ".repeat(10)
    ).repeat(10);
    bench_both(c, "ShapeLine/Combined Stress", &stress_text);
}

// fn bench_bidi_paragraphs_ascii(c: &mut Criterion) {
//     let ascii_text = "Simple ASCII text\nwith multiple lines\n".repeat(50);

//     c.bench_function("BidiParagraphs/ASCII", |b| {
//         b.iter(|| {
//             let paras = ct::BidiParagraphs::new(black_box(&ascii_text));
//             black_box(paras.count());
//         });
//     });
// }

// fn bench_bidi_paragraphs_mixed(c: &mut Criterion) {
//     let mixed_text = "Mixed English and العربية text\nwith multiple lines\n".repeat(30);

//     c.bench_function("BidiParagraphs/Mixed", |b| {
//         b.iter(|| {
//             let paras = ct::BidiParagraphs::new(black_box(&mixed_text));
//             black_box(paras.count());
//         });
//     });
// }

criterion_group!(
    benches,
    bench_ascii_fast_path,
    bench_bidi_processing,
    bench_lang_mixed,
    bench_layout_heavy,
    bench_combined_stress,
    // bench_bidi_paragraphs_ascii,
    // bench_bidi_paragraphs_mixed
);
criterion_main!(benches);
