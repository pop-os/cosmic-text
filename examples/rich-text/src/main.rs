// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::BorrowedWithFontSystem;
use cosmic_text::CacheKeyFlags;
use cosmic_text::Color;
use cosmic_text::Editor;
use cosmic_text::Shaping;
use cosmic_text::Style;
use cosmic_text::{
    Action, Attrs, Buffer, Edit, Family, FontSystem, Metrics, Motion, SwashCache, Weight,
};
use std::{num::NonZeroU32, rc::Rc, slice};
use tiny_skia::{Paint, PixmapMut, Rect, Transform};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

fn set_buffer_text(buffer: &mut BorrowedWithFontSystem<'_, Buffer>) {
    let attrs = Attrs::new();
    let serif_attrs = attrs.clone().family(Family::Serif);
    let mono_attrs = attrs.clone().family(Family::Monospace);
    let comic_attrs = attrs.clone().family(Family::Name("Comic Neue"));
    let inter_attrs = attrs.clone().family(Family::Name("Inter Variable"));

    let spans: &[(&str, Attrs)] = &[
        (
            "Font size 64 ",
            attrs.clone().metrics(Metrics::relative(64.0, 1.2)),
        ),
        (
            "\n\nFont size 8 \n\n",
            attrs.clone().metrics(Metrics::relative(8.0, 1.2)),
        ),
        (
            "Font size 20 ",
            attrs.clone().metrics(Metrics::relative(20.0, 1.2)),
        ),
        (
            "Font size 14 ",
            attrs.clone().metrics(Metrics::relative(14.0, 1.2)),
        ),
        (
            "Font size 48\n",
            attrs.clone().metrics(Metrics::relative(48.0, 1.2)),
        ),
        ("B", attrs.clone().weight(Weight::BOLD)),
        ("old ", attrs.clone()),
        ("I", attrs.clone().style(Style::Italic)),
        ("talic ", attrs.clone()),
        ("f", attrs.clone()),
        ("i ", attrs.clone()),
        ("f", attrs.clone().weight(Weight::BOLD)),
        ("i ", attrs.clone()),
        ("f", attrs.clone().style(Style::Italic)),
        ("i \n", attrs.clone()),
        ("Sans-Serif Normal ", attrs.clone()),
        ("Sans-Serif Bold ", attrs.clone().weight(Weight::BOLD)),
        ("Sans-Serif Italic ", attrs.clone().style(Style::Italic)),
        (
            "Sans-Serif Fake Italic ",
            attrs.clone().cache_key_flags(CacheKeyFlags::FAKE_ITALIC),
        ),
        (
            "Sans-Serif Bold Italic\n",
            attrs.clone().weight(Weight::BOLD).style(Style::Italic),
        ),
        ("Serif Normal ", serif_attrs.clone()),
        ("Serif Bold ", serif_attrs.clone().weight(Weight::BOLD)),
        ("Serif Italic ", serif_attrs.clone().style(Style::Italic)),
        (
            "Serif Bold Italic\n",
            serif_attrs
                .clone()
                .weight(Weight::BOLD)
                .style(Style::Italic),
        ),
        ("Mono Normal ", mono_attrs.clone()),
        ("Mono Bold ", mono_attrs.clone().weight(Weight::BOLD)),
        ("Mono Italic ", mono_attrs.clone().style(Style::Italic)),
        (
            "Mono Bold Italic\n",
            mono_attrs.clone().weight(Weight::BOLD).style(Style::Italic),
        ),
        ("Comic Normal ", comic_attrs.clone()),
        ("Comic Bold ", comic_attrs.clone().weight(Weight::BOLD)),
        ("Comic Italic ", comic_attrs.clone().style(Style::Italic)),
        (
            "Comic Bold Italic\n",
            comic_attrs
                .clone()
                .weight(Weight::BOLD)
                .style(Style::Italic),
        ),
        ("🌈", attrs.clone()),
        ("R", attrs.clone().color(Color::rgb(0xFF, 0x00, 0x00))),
        ("A", attrs.clone().color(Color::rgb(0xFF, 0x7F, 0x00))),
        ("I", attrs.clone().color(Color::rgb(0xFF, 0xFF, 0x00))),
        ("N", attrs.clone().color(Color::rgb(0x00, 0xFF, 0x00))),
        ("B", attrs.clone().color(Color::rgb(0x00, 0x00, 0xFF))),
        ("O", attrs.clone().color(Color::rgb(0x4B, 0x00, 0x82))),
        ("W ", attrs.clone().color(Color::rgb(0x94, 0x00, 0xD3))),
        ("Red ", attrs.clone().color(Color::rgb(0xFF, 0x00, 0x00))),
        ("Orange ", attrs.clone().color(Color::rgb(0xFF, 0x7F, 0x00))),
        ("Yellow ", attrs.clone().color(Color::rgb(0xFF, 0xFF, 0x00))),
        ("Green ", attrs.clone().color(Color::rgb(0x00, 0xFF, 0x00))),
        ("Blue ", attrs.clone().color(Color::rgb(0x00, 0x00, 0xFF))),
        ("Indigo ", attrs.clone().color(Color::rgb(0x4B, 0x00, 0x82))),
        ("Violet ", attrs.clone().color(Color::rgb(0x94, 0x00, 0xD3))),
        ("U", attrs.clone().color(Color::rgb(0x94, 0x00, 0xD3))),
        ("N", attrs.clone().color(Color::rgb(0x4B, 0x00, 0x82))),
        ("I", attrs.clone().color(Color::rgb(0x00, 0x00, 0xFF))),
        ("C", attrs.clone().color(Color::rgb(0x00, 0xFF, 0x00))),
        ("O", attrs.clone().color(Color::rgb(0xFF, 0xFF, 0x00))),
        ("R", attrs.clone().color(Color::rgb(0xFF, 0x7F, 0x00))),
        ("N", attrs.clone().color(Color::rgb(0xFF, 0x00, 0x00))),
        ("🦄\n", attrs.clone()),
        (
            "生活,삶,जिंदगी 😀\n",
            attrs.clone().color(Color::rgb(0xFF, 0x00, 0x00)),
        ),
        ("Hinting enabled, ", attrs.clone()),
        (
            "Hinting disabled\n",
            attrs
                .clone()
                .cache_key_flags(CacheKeyFlags::DISABLE_HINTING),
        ),
        (
            "Inter Variable: 400 ",
            inter_attrs.clone().weight(Weight(400)),
        ),
        ("200 ", inter_attrs.clone().weight(Weight(200))),
        ("250 ", inter_attrs.clone().weight(Weight(250))),
        ("300\n", inter_attrs.clone().weight(Weight(300))),
        (
            "Inter Variable Italic: 400 ",
            inter_attrs.clone().weight(Weight(400)).style(Style::Italic),
        ),
        (
            "800",
            inter_attrs.clone().weight(Weight(800)).style(Style::Italic),
        ),
    ];

    buffer.set_rich_text(
        spans.iter().map(|(text, attrs)| (*text, attrs.clone())),
        &attrs,
        Shaping::Advanced,
        None,
    );
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = Rc::new(WindowBuilder::new().build(&event_loop).unwrap());
    let context = softbuffer::Context::new(window.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
    let mut font_system = FontSystem::new();
    let inter_variable = include_bytes!("../../../fonts/InterVariable.ttf");
    font_system.db_mut().load_font_data(inter_variable.to_vec());
    let inter_variable_italic = include_bytes!("../../../fonts/InterVariable-Italic.ttf");
    font_system
        .db_mut()
        .load_font_data(inter_variable_italic.to_vec());
    let mut swash_cache = SwashCache::new();

    let mut display_scale = window.scale_factor() as f32;
    let metrics = Metrics::new(32.0, 44.0);
    let mut editor = Editor::new(Buffer::new_empty(metrics.scale(display_scale)));
    let mut editor = editor.borrow_with(&mut font_system);
    editor.with_buffer_mut(|buffer| {
        buffer.set_size(
            Some(window.inner_size().width as f32),
            Some(window.inner_size().height as f32),
        )
    });
    editor.with_buffer_mut(set_buffer_text);

    let mut ctrl_pressed = false;
    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;
    let mut mouse_left = ElementState::Released;

    let bg_color = tiny_skia::Color::from_rgba8(0x34, 0x34, 0x34, 0xFF);
    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);
    let cursor_color = Color::rgb(0xFF, 0xFF, 0xFF);
    let selection_color = Color::rgba(0xFF, 0xFF, 0xFF, 0x33);
    let selected_text_color = Color::rgb(0xA0, 0xA0, 0xFF);

    event_loop
        .run(|event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            let Event::WindowEvent { window_id, event } = event else {
                return;
            };

            match event {
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    log::info!("Updated scale factor for {window_id:?}");

                    display_scale = scale_factor as f32;
                    editor
                        .with_buffer_mut(|buffer| buffer.set_metrics(metrics.scale(display_scale)));

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
                    pixmap.fill(bg_color);

                    editor.with_buffer_mut(|buffer| {
                        buffer.set_size(Some(width as f32), Some(height as f32))
                    });

                    let mut paint = Paint {
                        anti_alias: false,
                        ..Default::default()
                    };
                    editor.shape_as_needed(true);

                    editor.draw(
                        &mut swash_cache,
                        font_color,
                        cursor_color,
                        selection_color,
                        selected_text_color,
                        |x, y, w, h, color| {
                            // Note: due to softbuffer and tiny_skia having incompatible internal color representations we swap
                            // the red and blue channels here
                            paint.set_color_rgba8(color.b(), color.g(), color.r(), color.a());
                            pixmap.fill_rect(
                                Rect::from_xywh(x as f32, y as f32, w as f32, h as f32).unwrap(),
                                &paint,
                                Transform::identity(),
                                None,
                            );
                        },
                    );

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
                            Key::Named(NamedKey::End) => editor.action(Action::Motion(Motion::End)),
                            Key::Named(NamedKey::PageUp) => {
                                editor.action(Action::Motion(Motion::PageUp))
                            }
                            Key::Named(NamedKey::PageDown) => {
                                editor.action(Action::Motion(Motion::PageDown))
                            }
                            Key::Named(NamedKey::Escape) => editor.action(Action::Escape),
                            Key::Named(NamedKey::Enter) => editor.action(Action::Enter),
                            Key::Named(NamedKey::Backspace) => editor.action(Action::Backspace),
                            Key::Named(NamedKey::Delete) => editor.action(Action::Delete),
                            Key::Named(key) => {
                                if let Some(text) = key.to_text() {
                                    for c in text.chars() {
                                        editor.action(Action::Insert(c));
                                    }
                                }
                            }
                            Key::Character(text) => {
                                if !ctrl_pressed {
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
                            editor.action(Action::Scroll { pixels: -20.0 });
                        } else if mouse_y - 5.0 >= window.inner_size().height as f64 {
                            editor.action(Action::Scroll { pixels: 20.0 });
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
                        if state == ElementState::Pressed && mouse_left == ElementState::Released {
                            editor.action(Action::Click {
                                x: mouse_x /*- line_x*/ as i32,
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
                    let pixel_delta = match delta {
                        MouseScrollDelta::LineDelta(_x, y) => y * 20.0,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { x: _, y }) => y as f32,
                    };
                    if pixel_delta != 0.0 {
                        editor.action(Action::Scroll {
                            pixels: -pixel_delta,
                        });
                    }
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => {
                    //TODO: just close one window
                    elwt.exit();
                }
                _ => {}
            }
        })
        .unwrap();
}
