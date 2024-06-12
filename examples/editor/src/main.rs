// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Action, Attrs, Buffer, Edit, Family, FontSystem, Metrics, Motion, SwashCache, SyntaxEditor,
    SyntaxSystem,
};
use std::{env, fs, num::NonZeroU32, rc::Rc, slice};
use tiny_skia::{Paint, PixmapMut, Rect, Transform};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

fn main() {
    env_logger::init();

    let path = env::args().nth(1).unwrap_or(String::new());

    let event_loop = EventLoop::new().unwrap();
    let window = Rc::new(WindowBuilder::new().build(&event_loop).unwrap());
    let context = softbuffer::Context::new(window.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
    let mut font_system = FontSystem::new();
    let syntax_system = SyntaxSystem::new();
    let mut swash_cache = SwashCache::new();

    let mut display_scale = window.scale_factor() as f32;

    let scrollbar_width = 12.0;
    let font_sizes = [
        Metrics::new(10.0, 14.0), // Caption
        Metrics::new(14.0, 20.0), // Body
        Metrics::new(20.0, 28.0), // Title 4
        Metrics::new(24.0, 32.0), // Title 3
        Metrics::new(28.0, 36.0), // Title 2
        Metrics::new(32.0, 44.0), // Title 1
    ];
    let font_size_default = 1; // Body
    let mut font_size_i = font_size_default;

    let mut editor = SyntaxEditor::new(
        Buffer::new(
            &mut font_system,
            font_sizes[font_size_i].scale(display_scale),
        ),
        &syntax_system,
        "base16-eighties.dark",
    )
    .unwrap();
    let mut editor = editor.borrow_with(&mut font_system);

    let attrs = Attrs::new().family(Family::Monospace);

    match editor.load_text(&path, attrs) {
        Ok(()) => (),
        Err(err) => {
            log::error!("failed to load {:?}: {}", path, err);
        }
    }

    let mut ctrl_pressed = false;
    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;
    let mut mouse_left = ElementState::Released;
    let mut unapplied_scroll_delta = 0.0;

    event_loop
        .run(|event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            match event {
                Event::WindowEvent { window_id, event } => {
                    match event {
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            log::info!("Updated scale factor for {window_id:?}");

                            display_scale = scale_factor as f32;
                            editor.with_buffer_mut(|buffer| {
                                buffer.set_metrics(font_sizes[font_size_i].scale(display_scale))
                            });

                            window.request_redraw();
                        }
                        WindowEvent::RedrawRequested => {
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
                            pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0xFF));

                            editor.with_buffer_mut(|buffer| {
                                buffer.set_size(
                                    Some(width as f32 - scrollbar_width * display_scale),
                                    Some(height as f32),
                                )
                            });

                            let mut paint = Paint::default();
                            paint.anti_alias = false;
                            editor.shape_as_needed(true);
                            editor.draw(&mut swash_cache, |x, y, w, h, color| {
                                // Note: due to softbuffer and tiny_skia having incompatible internal color representations we swap
                                // the red and blue channels here
                                paint.set_color_rgba8(color.b(), color.g(), color.r(), color.a());
                                pixmap.fill_rect(
                                    Rect::from_xywh(x as f32, y as f32, w as f32, h as f32)
                                        .unwrap(),
                                    &paint,
                                    Transform::identity(),
                                    None,
                                );
                            });
                            if let Some((x, y)) = editor.cursor_position() {
                                window.set_ime_cursor_area(
                                    PhysicalPosition::new(x, y),
                                    PhysicalSize::new(20, 20),
                                );
                            }

                            // Draw scrollbar
                            {
                                let mut start_line_opt = None;
                                let mut end_line = 0;
                                editor.with_buffer(|buffer| {
                                    for run in buffer.layout_runs() {
                                        end_line = run.line_i;
                                        if start_line_opt.is_none() {
                                            start_line_opt = Some(end_line);
                                        }
                                    }
                                });

                                let start_line = start_line_opt.unwrap_or(end_line);
                                let lines = editor.with_buffer(|buffer| buffer.lines.len());
                                let start_y = (start_line * height as usize) / lines;
                                let end_y = (end_line * height as usize) / lines;
                                paint.set_color_rgba8(0xFF, 0xFF, 0xFF, 0x40);
                                if end_y > start_y {
                                    pixmap.fill_rect(
                                        Rect::from_xywh(
                                            width as f32 - scrollbar_width * display_scale,
                                            start_y as f32,
                                            scrollbar_width * display_scale,
                                            (end_y - start_y) as f32,
                                        )
                                        .unwrap(),
                                        &paint,
                                        Transform::identity(),
                                        None,
                                    );
                                }
                            }

                            surface_buffer.present().unwrap();
                        }
                        WindowEvent::ModifiersChanged(modifiers) => {
                            ctrl_pressed = modifiers.state().control_key()
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            let KeyEvent {
                                logical_key, state, ..
                            } = event;

                            if state.is_pressed() {
                                match logical_key {
                                    Key::Named(NamedKey::ArrowLeft) => {
                                        editor.action(Action::Motion(Motion::Left))
                                    }
                                    Key::Named(NamedKey::ArrowRight) => {
                                        editor.action(Action::Motion(Motion::Right))
                                    }
                                    Key::Named(NamedKey::ArrowUp) => {
                                        editor.action(Action::Motion(Motion::Up))
                                    }
                                    Key::Named(NamedKey::ArrowDown) => {
                                        editor.action(Action::Motion(Motion::Down))
                                    }
                                    Key::Named(NamedKey::Home) => {
                                        editor.action(Action::Motion(Motion::Home))
                                    }
                                    Key::Named(NamedKey::End) => {
                                        editor.action(Action::Motion(Motion::End))
                                    }
                                    Key::Named(NamedKey::PageUp) => {
                                        editor.action(Action::Motion(Motion::PageUp))
                                    }
                                    Key::Named(NamedKey::PageDown) => {
                                        editor.action(Action::Motion(Motion::PageDown))
                                    }
                                    Key::Named(NamedKey::Escape) => editor.action(Action::Escape),
                                    Key::Named(NamedKey::Enter) => editor.action(Action::Enter),
                                    Key::Named(NamedKey::Backspace) => {
                                        editor.action(Action::Backspace)
                                    }
                                    Key::Named(NamedKey::Delete) => editor.action(Action::Delete),
                                    Key::Named(key) => {
                                        if let Some(text) = key.to_text() {
                                            for c in text.chars() {
                                                editor.action(Action::Insert(c));
                                            }
                                        }
                                    }
                                    Key::Character(text) => {
                                        if ctrl_pressed {
                                            match &*text {
                                                "0" => {
                                                    font_size_i = font_size_default;
                                                    editor.with_buffer_mut(|buffer| {
                                                        buffer.set_metrics(
                                                            font_sizes[font_size_i]
                                                                .scale(display_scale),
                                                        )
                                                    });
                                                }
                                                "-" => {
                                                    if font_size_i > 0 {
                                                        font_size_i -= 1;
                                                        editor.with_buffer_mut(|buffer| {
                                                            buffer.set_metrics(
                                                                font_sizes[font_size_i]
                                                                    .scale(display_scale),
                                                            )
                                                        });
                                                    }
                                                }
                                                "=" => {
                                                    if font_size_i + 1 < font_sizes.len() {
                                                        font_size_i += 1;
                                                        editor.with_buffer_mut(|buffer| {
                                                            buffer.set_metrics(
                                                                font_sizes[font_size_i]
                                                                    .scale(display_scale),
                                                            )
                                                        });
                                                    }
                                                }
                                                "s" => {
                                                    let mut text = String::new();
                                                    editor.with_buffer(|buffer| {
                                                        for line in buffer.lines.iter() {
                                                            text.push_str(line.text());
                                                            text.push_str(line.ending().as_str());
                                                        }
                                                    });
                                                    fs::write(&path, &text).unwrap();
                                                    log::info!("saved {:?}", path);
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            for c in text.chars() {
                                                editor.action(Action::Insert(c));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                window.request_redraw();
                            }
                        }
                        WindowEvent::CursorMoved {
                            device_id: _,
                            position,
                        } => {
                            // Update saved mouse position for use when handling click events
                            mouse_x = position.x;
                            mouse_y = position.y;

                            // Implement dragging
                            if mouse_left.is_pressed() {
                                // Execute Drag editor action (update selection)
                                editor.action(Action::Drag {
                                    x: position.x as i32,
                                    y: position.y as i32,
                                });

                                // Scroll if cursor is near edge of window while dragging
                                if mouse_y <= 5.0 {
                                    editor.action(Action::Scroll { lines: -1 });
                                } else if mouse_y - 5.0 >= window.inner_size().height as f64 {
                                    editor.action(Action::Scroll { lines: 1 });
                                }

                                window.request_redraw();
                            }
                        }
                        WindowEvent::MouseInput {
                            device_id: _,
                            state,
                            button,
                        } => {
                            if button == MouseButton::Left {
                                if state == ElementState::Pressed
                                    && mouse_left == ElementState::Released
                                {
                                    editor.action(Action::Click {
                                        x: mouse_x as i32,
                                        y: mouse_y as i32,
                                    });
                                    window.request_redraw();
                                }
                                mouse_left = state;
                            }
                        }
                        WindowEvent::MouseWheel {
                            device_id: _,
                            delta,
                            phase: _,
                        } => {
                            let line_delta = match delta {
                                MouseScrollDelta::LineDelta(_x, y) => y as i32,
                                MouseScrollDelta::PixelDelta(PhysicalPosition { x: _, y }) => {
                                    unapplied_scroll_delta += y;
                                    let line_delta = (unapplied_scroll_delta / 20.0).floor();
                                    unapplied_scroll_delta -= line_delta * 20.0;
                                    line_delta as i32
                                }
                            };
                            if line_delta != 0 {
                                editor.action(Action::Scroll { lines: -line_delta });
                            }
                            window.request_redraw();
                        }
                        WindowEvent::CloseRequested => {
                            //TODO: just close one window
                            elwt.exit();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        })
        .unwrap();
}
