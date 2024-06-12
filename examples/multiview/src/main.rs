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
                            |x, y, w, h, color| {
                                paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
                                pixmap.fill_rect(
                                    Rect::from_xywh(x as f32, y as f32, w as f32, h as f32)
                                        .unwrap(),
                                    &paint,
                                    transform,
                                    None,
                                );
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
