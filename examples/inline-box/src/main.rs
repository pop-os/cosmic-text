// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Align, Attrs, BorrowedWithFontSystem, Buffer, Color, FontSystem, InlineAlignment, InlineBox,
    InlineBoxId, Metrics, RichSpan, Shaping, SwashCache,
};
use std::{collections::HashMap, num::NonZeroU32, rc::Rc, slice};
use tiny_skia::{FilterQuality, Paint, Pixmap, PixmapMut, PixmapPaint, Rect, Transform};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

struct Image {
    name: &'static str,
    png: &'static [u8],
    width: f32,
    height: f32,
}

const IMAGES: &[Image] = &[
    Image {
        name: "cosmic-logo",
        png: include_bytes!("../assets/cosmic-logo.png"),
        width: 220.0,
        height: 28.0,
    },
    Image {
        name: "screenshot-arabic",
        png: include_bytes!("../assets/screenshot-arabic.png"),
        width: 360.0,
        height: 284.0,
    },
    Image {
        name: "pop-os",
        png: include_bytes!("../assets/pop-os.png"),
        width: 22.0,
        height: 22.0,
    },
];

/// What each inline box shows, keyed by the id
type Boxes = HashMap<InlineBoxId, (usize, InlineAlignment)>;

fn set_buffer_text(buffer: &mut BorrowedWithFontSystem<'_, Buffer>, scale: f32) -> Boxes {
    let attrs = Attrs::new();
    let text = |s: &'static str| RichSpan::Text(s, attrs.clone());
    let mut boxes = Boxes::new();
    let mut image = |img: usize, align: InlineAlignment| {
        let id = buffer.next_inline_box_id();
        boxes.insert(id, (img, align));
        let align = match align {
            InlineAlignment::Baseline { offset } => InlineAlignment::Baseline {
                offset: offset * scale,
            },
            other => other,
        };
        let image = &IMAGES[img];
        let (w, h) = (image.width * scale, image.height * scale);
        RichSpan::InlineBox(InlineBox::new(id, w, h, align))
    };

    let spans = [
        text("This is "),
        image(0, InlineAlignment::TextCenter),
        text("-text. It supports bidirectional text, as shown in this screenshot:\n\n"),
        image(1, InlineAlignment::TextCenter),
        text(
            "\n\nIt also supports inline boxes, which reserve space in the text flow for \
             anything that is not text. Here is the same icon with every alignment option: \
             AboveBaseline\u{a0}",
        ),
        image(2, InlineAlignment::AboveBaseline),
        text(", BelowBaseline\u{a0}"),
        image(2, InlineAlignment::BelowBaseline),
        text(", Top\u{a0}"),
        image(2, InlineAlignment::TextTop),
        text(", Bottom\u{a0}"),
        image(2, InlineAlignment::TextBottom),
        text(", Middle\u{a0}"),
        image(2, InlineAlignment::Middle),
        text(", Center\u{a0}"),
        image(2, InlineAlignment::TextCenter),
        text(" and a custom Baseline with offset:6"),
        image(2, InlineAlignment::Baseline { offset: 6.0 }),
        text(
            ", which puts the box's own baseline 6 px below its top edge. Boxes wrap like \
             words and can be hit-tested: click any image to print its name and \
             alignment.\n\n",
        ),
        text(
            "این جمله کاملاً به فارسی و از راست به چپ است. جعبه‌های درون‌خطی، مانند این \
             تصویر ",
        ),
        image(0, InlineAlignment::TextCenter),
        text(" هم در همین جهت چیده می‌شوند."),
    ];
    buffer.set_rich_text_with_boxes(spans, &attrs, Shaping::Advanced, None);

    // Center the line that holds nothing but the screenshot.
    for line in buffer.lines.iter_mut() {
        if line.text() == "\u{FFFC}" {
            line.set_align(Some(Align::Center));
        }
    }
    boxes
}

fn main() {
    env_logger::init();

    let images: Vec<Pixmap> = IMAGES
        .iter()
        .map(|img| {
            let mut pixmap = Pixmap::decode_png(img.png).unwrap();
            // softbuffer stores BGRA while tiny-skia decodes RGBA, so swap once here
            for px in pixmap.data_mut().as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
            pixmap
        })
        .collect();

    let event_loop = EventLoop::new().unwrap();
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("cosmic-text inline boxes")
            .with_inner_size(LogicalSize::new(1000.0, 780.0))
            .build(&event_loop)
            .unwrap(),
    );
    let context = softbuffer::Context::new(window.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();

    let metrics = Metrics::new(14.0, 20.0);
    let mut scale = window.scale_factor() as f32;
    let mut buffer = Buffer::new(&mut font_system, metrics.scale(scale));
    let mut boxes = set_buffer_text(&mut buffer.borrow_with(&mut font_system), scale);

    let bg_color = tiny_skia::Color::from_rgba8(0x34, 0x34, 0x34, 0xFF);
    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);
    let image_paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..Default::default()
    };
    let mut mouse = (0.0, 0.0);

    event_loop
        .run(|event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            let Event::WindowEvent { event, .. } = event else {
                return;
            };

            match event {
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    scale = scale_factor as f32;
                    buffer.set_metrics(metrics.scale(scale));
                    boxes = set_buffer_text(&mut buffer.borrow_with(&mut font_system), scale);
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    let size = window.inner_size();
                    let (width, height) = (size.width, size.height);
                    surface
                        .resize(
                            NonZeroU32::new(width).unwrap(),
                            NonZeroU32::new(height).unwrap(),
                        )
                        .unwrap();

                    let mut surface_buffer = surface.buffer_mut().unwrap();
                    let surface_buffer_u8 = unsafe {
                        slice::from_raw_parts_mut(
                            surface_buffer.as_mut_ptr() as *mut u8,
                            surface_buffer.len() * 4,
                        )
                    };
                    let mut pixmap =
                        PixmapMut::from_bytes(surface_buffer_u8, width, height).unwrap();
                    pixmap.fill(bg_color);

                    buffer.set_size(Some(width as f32), Some(height as f32));

                    let mut paint = Paint {
                        anti_alias: false,
                        ..Default::default()
                    };
                    buffer.draw(
                        &mut font_system,
                        &mut swash_cache,
                        font_color,
                        |x, y, w, h, color| {
                            paint.set_color_rgba8(color.b(), color.g(), color.r(), color.a());
                            pixmap.fill_rect(
                                Rect::from_xywh(x as f32, y as f32, w as f32, h as f32).unwrap(),
                                &paint,
                                Transform::identity(),
                                None,
                            );
                        },
                    );

                    // `draw` skips the box placeholders; scale each image into the
                    // space its box reserved. `span.y` is relative to the baseline.
                    for run in buffer.layout_runs() {
                        for span in run.inline_boxes {
                            let (img, _) = boxes[&span.id];
                            let image = images[img].as_ref();
                            let transform = Transform::from_row(
                                span.width / image.width() as f32,
                                0.0,
                                0.0,
                                span.height / image.height() as f32,
                                span.x,
                                run.line_y + span.y,
                            );
                            pixmap.draw_pixmap(0, 0, image, &image_paint, transform, None);
                        }
                    }

                    surface_buffer.present().unwrap();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    mouse = (position.x as f32, position.y as f32);
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    let hit = buffer
                        .borrow_with(&mut font_system)
                        .hit_inline_box(mouse.0, mouse.1);
                    match hit {
                        Some(hit) => {
                            let (img, align) = boxes[&hit.id];
                            println!(
                                "clicked {} aligned {:?} at ({:.0}, {:.0}) inside the box",
                                IMAGES[img].name, align, hit.local_x, hit.local_y
                            );
                        }
                        None => println!("clicked text"),
                    }
                }
                WindowEvent::Resized(_) => window.request_redraw(),
                WindowEvent::CloseRequested => elwt.exit(),
                _ => {}
            }
        })
        .unwrap();
}
