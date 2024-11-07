// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Action, Attrs, Buffer, Edit, Family, FontSystem, Metrics, Scroll, Shaping, SwashCache,
};
use std::{collections::HashMap, env, fs, num::NonZeroU32, rc::Rc, slice};
use tiny_skia::{Color, Paint, PixmapMut, Rect, Transform};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window as WinitWindow, WindowBuilder},
};

fn main() {
    env_logger::init();

    let path = if let Some(arg) = env::args().nth(1) {
        arg
    } else {
        "sample/hello.txt".to_string()
    };

    let mut font_system = FontSystem::new();

    let mut swash_cache = SwashCache::new();

    let mut buffer = Buffer::new_empty(Metrics::new(14.0, 20.0));

    let mut buffer = buffer.borrow_with(&mut font_system);

    let attrs = Attrs::new().family(Family::Monospace);
    match fs::read_to_string(&path) {
        Ok(text) => buffer.set_text(&text, attrs, Shaping::Advanced),
        Err(err) => {
            log::error!("failed to load {:?}: {}", path, err);
        }
    }

    let event_loop = EventLoop::new().unwrap();

    struct Window {
        window: Rc<WinitWindow>,
        context: softbuffer::Context<Rc<WinitWindow>>,
        surface: softbuffer::Surface<Rc<WinitWindow>, Rc<WinitWindow>>,
        scroll: Scroll,
    }
    let mut windows = HashMap::new();
    for _ in 0..2 {
        let window = Rc::new(WindowBuilder::new().build(&event_loop).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        windows.insert(
            window.id(),
            Window {
                window,
                context,
                surface,
                scroll: Scroll::default(),
            },
        );
    }

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            match event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::RedrawRequested,
                } => {
                    if let Some(Window {
                        window,
                        surface,
                        scroll,
                        ..
                    }) = windows.get_mut(&window_id)
                    {
                        let (width, height) = {
                            let size = window.inner_size();
                            (size.width, size.height)
                        };
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
                        pixmap.fill(Color::from_rgba8(0, 0, 0, 0xFF));

                        // Set scroll to view scroll
                        buffer.set_scroll(*scroll);
                        // Set size, will relayout and shape until scroll if changed
                        buffer.set_size(Some(width as f32), Some(height as f32));
                        // Shape until scroll, ensures scroll is clamped
                        //TODO: ability to prune with multiple views?
                        buffer.shape_until_scroll(true);
                        // Update scroll after buffer clamps it
                        *scroll = buffer.scroll();

                        let mut paint = Paint::default();
                        paint.anti_alias = false;
                        let transform = Transform::identity();
                        buffer.draw(
                            &mut swash_cache,
                            cosmic_text::Color::rgb(0xFF, 0xFF, 0xFF),
                            |x, y, w, h, color, subpixel_mask| {
                                if let Some(mask) = subpixel_mask {
                                    // Subpixel mask must be manually applied
                                    //TODO: just clamp x, y, w, and h?
                                    if x < 0 || y < 0 {
                                        return;
                                    }
                                    let width = pixmap.width();
                                    if x as u32 + w > width {
                                        return;
                                    }
                                    let height = pixmap.height();
                                    if y as u32 + h > height {
                                        return;
                                    }
                                    //TODO: simd?
                                    let color_r = color.r() as u32;
                                    let color_g = color.g() as u32;
                                    let color_b = color.b() as u32;
                                    let color_a = color.a() as u32;
                                    let mask_r = ((mask.r() as u32) * color_a) >> 8;
                                    let mask_g = ((mask.g() as u32) * color_a) >> 8;
                                    let mask_b = ((mask.b() as u32) * color_a) >> 8;
                                    let pixels = pixmap.pixels_mut();
                                    for row in 0..h {
                                        let row_i = (y as u32 + row) * width;
                                        for col in 0..w {
                                            let i = (row_i + x as u32 + col) as usize;
                                            let pixel = &mut pixels[i];
                                            // Note: due to softbuffer and tiny_skia having incompatible internal color representations we swap
                                            // the red and blue channels here
                                            let pixel_r = pixel.blue() as u32;
                                            let pixel_g = pixel.green() as u32;
                                            let pixel_b = pixel.red() as u32;
                                            let r = ((pixel_r * (255 - mask_r))
                                                + (color_r * mask_r))
                                                >> 8;
                                            let g = ((pixel_g * (255 - mask_g))
                                                + (color_g * mask_g))
                                                >> 8;
                                            let b = ((pixel_b * (255 - mask_b))
                                                + (color_b * mask_b))
                                                >> 8;
                                            // Note: due to softbuffer and tiny_skia having incompatible internal color representations we swap
                                            // the red and blue channels here
                                            *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
                                                b as u8, g as u8, r as u8, 0xFF,
                                            )
                                            .unwrap();
                                        }
                                    }
                                } else {
                                    // Note: due to softbuffer and tiny_skia having incompatible internal color representations we swap
                                    // the red and blue channels here
                                    paint.set_color_rgba8(
                                        color.b(),
                                        color.g(),
                                        color.r(),
                                        color.a(),
                                    );
                                    pixmap.fill_rect(
                                        Rect::from_xywh(x as f32, y as f32, w as f32, h as f32)
                                            .unwrap(),
                                        &paint,
                                        transform,
                                        None,
                                    );
                                }
                            },
                        );

                        surface_buffer.present().unwrap();
                    }
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    logical_key,
                                    text,
                                    state,
                                    ..
                                },
                            ..
                        },
                    window_id,
                } => {
                    if let Some(Window { window, scroll, .. }) = windows.get_mut(&window_id) {
                        if state == ElementState::Pressed {
                            match logical_key {
                                Key::Named(NamedKey::ArrowDown) => {
                                    scroll.vertical += buffer.metrics().line_height;
                                }
                                Key::Named(NamedKey::ArrowUp) => {
                                    scroll.vertical -= buffer.metrics().line_height;
                                }
                                Key::Named(NamedKey::PageDown) => {
                                    scroll.vertical += buffer.size().1.unwrap_or(0.0);
                                }
                                Key::Named(NamedKey::PageUp) => {
                                    scroll.vertical -= buffer.size().1.unwrap_or(0.0);
                                }
                                _ => {}
                            }
                        }
                        println!("{:?} {:?} {:?}", logical_key, text, state);
                        window.request_redraw();
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id: _,
                } => {
                    //TODO: just close one window
                    elwt.exit();
                }
                _ => {}
            }
        })
        .unwrap();
}
