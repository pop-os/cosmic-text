use cosmic_text as ct;
use criterion::black_box;
use linebender_resource_handle::Blob;
use std::borrow::Cow;
use std::sync::Arc;

const DEJA_SANS_FONT: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");

const USE_DEJA: bool = false;

pub(crate) struct CtBencher {
    font_system: ct::FontSystem,
    buffer: ct::Buffer,
}

impl CtBencher {
    pub(crate) fn new(font_size: f32, line_height: f32, wrap: ct::Wrap, width: f32) -> Self {
        let mut font_system = ct::FontSystem::new();

        // DEJA
        if USE_DEJA {
            font_system.db_mut().load_font_data(DEJA_SANS_FONT.to_vec());
        }

        let mut buffer =
            ct::Buffer::new(&mut font_system, ct::Metrics::new(font_size, line_height));
        buffer.set_size(&mut font_system, Some(width), None);
        buffer.set_wrap(&mut font_system, wrap);
        Self {
            font_system,
            buffer,
        }
    }

    pub(crate) fn shape_and_layout_text(&mut self, text: &str, shaping_mode: ct::Shaping) {
        self.buffer.set_text(
            &mut self.font_system,
            black_box(&text),
            &if USE_DEJA {
                ct::Attrs::new().family(fontdb::Family::Name("DejaVu Sans"))
            } else {
                ct::Attrs::new()
            },
            shaping_mode,
            None,
        );
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        black_box(&mut self.buffer);
    }
}

pub(crate) struct ParleyBencher {
    font_ctx: parley::FontContext,
    layout_ctx: parley::LayoutContext,
    font_size: f32,
    line_height: f32,
    width: f32,
    overflow_wrap: parley::OverflowWrap,
}

impl ParleyBencher {
    pub(crate) fn new(font_size: f32, line_height: f32, wrap: ct::Wrap, width: f32) -> Self {
        let mut font_ctx = parley::FontContext::new();
        let layout_ctx = parley::LayoutContext::new();

        // DEJA
        if USE_DEJA {
            font_ctx
                .collection
                .register_fonts(Blob::new(Arc::new(DEJA_SANS_FONT)), None);
        }

        let width = match wrap {
            ct::Wrap::None => f32::MAX,
            _ => width,
        };

        let overflow_wrap = match wrap {
            ct::Wrap::None => parley::OverflowWrap::Anywhere,
            ct::Wrap::Glyph => parley::OverflowWrap::Anywhere,
            ct::Wrap::Word => parley::OverflowWrap::Normal,
            ct::Wrap::WordOrGlyph => parley::OverflowWrap::Normal,
        };

        Self {
            font_ctx,
            layout_ctx,
            font_size,
            line_height,
            width,
            overflow_wrap,
        }
    }

    pub(crate) fn shape_and_layout_text(&mut self, text: &str) {
        let mut builder =
            self.layout_ctx
                .ranged_builder(&mut self.font_ctx, black_box(&text), 1.0, false);
        builder.push_default(parley::StyleProperty::FontSize(self.font_size));
        builder.push_default(parley::LineHeight::Absolute(self.line_height));
        builder.push_default(parley::StyleProperty::OverflowWrap(self.overflow_wrap));
        if USE_DEJA {
            builder.push_default(parley::style::FontFamily::Named(Cow::Borrowed(
                "DejaVu Sans",
            )));
        }
        let mut layout = builder.build(black_box(&text));
        layout.break_all_lines(Some(self.width));
        black_box(&mut layout);
    }
}
