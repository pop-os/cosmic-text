use cosmic_text as ct;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_decoration_toggle(c: &mut Criterion) {
    let mut fs = ct::FontSystem::new();
    let mut buffer = ct::Buffer::new(&mut fs, ct::Metrics::new(14.0, 20.0));
    buffer.set_size(Some(600.0), None);

    let text = "The quick brown fox jumps over the lazy dog. ".repeat(8);
    buffer.set_text(&text, &ct::Attrs::new(), ct::Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut fs, false);

    let plain = ct::AttrsList::new(&ct::Attrs::new());
    let mut underline = ct::Attrs::new();
    underline.text_decoration.underline = ct::UnderlineStyle::Single;
    let mut underlined = ct::AttrsList::new(&ct::Attrs::new());
    underlined.add_span(0..20, &underline);

    let mut on = false;
    c.bench_function("ShapeLine/Decoration Toggle", |b| {
        b.iter(|| {
            on = !on;
            buffer.lines[0].set_attrs_list(if on {
                underlined.clone()
            } else {
                plain.clone()
            });
            buffer.shape_until_scroll(&mut fs, false);
            black_box(buffer.layout_runs().count());
        });
    });
}

criterion_group!(benches, bench_decoration_toggle);
criterion_main!(benches);
