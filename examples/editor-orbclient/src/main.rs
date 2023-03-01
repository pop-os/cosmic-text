// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Action, Attrs, Buffer, Edit, Family, FontSystem, Metrics, SwashCache, SyntaxEditor,
    SyntaxSystem,
};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{
    env, thread,
    time::{Duration, Instant},
};

fn main() {
    env_logger::init();

    let path = if let Some(arg) = env::args().nth(1) {
        arg.clone()
    } else {
        String::new()
    };

    let display_scale = match orbclient::get_display_size() {
        Ok((w, h)) => {
            log::info!("Display size: {}, {}", w, h);
            (h as f32 / 1600.0) + 1.0
        }
        Err(err) => {
            log::warn!("Failed to get display size: {}", err);
            1.0
        }
    };

    let mut window = Window::new_flags(
        -1,
        -1,
        1024 * display_scale as u32,
        768 * display_scale as u32,
        &format!("COSMIC Text - {}", path),
        &[WindowFlag::Resizable],
    )
    .unwrap();

    let font_system = FontSystem::new();

    let syntax_system = SyntaxSystem::new();

    let font_sizes = [
        Metrics::new(10.0, 14.0).scale(display_scale), // Caption
        Metrics::new(14.0, 20.0).scale(display_scale), // Body
        Metrics::new(20.0, 28.0).scale(display_scale), // Title 4
        Metrics::new(24.0, 32.0).scale(display_scale), // Title 3
        Metrics::new(28.0, 36.0).scale(display_scale), // Title 2
        Metrics::new(32.0, 44.0).scale(display_scale), // Title 1
    ];
    let font_size_default = 1; // Body
    let mut font_size_i = font_size_default;

    let line_x = 8.0 * display_scale;

    let mut editor = SyntaxEditor::new(
        Buffer::new(&font_system, font_sizes[font_size_i]),
        &syntax_system,
        "base16-eighties.dark",
    )
    .unwrap();

    #[cfg(feature = "vi")]
    let mut editor = cosmic_text::ViEditor::new(editor);

    editor
        .buffer_mut()
        .set_size(window.width() as f32 - line_x * 2.0, window.height() as f32);

    let attrs = Attrs::new().monospaced(true).family(Family::Monospace);
    match editor.load_text(&path, attrs) {
        Ok(()) => (),
        Err(err) => {
            log::error!("failed to load {:?}: {}", path, err);
        }
    }

    let mut swash_cache = SwashCache::new(&font_system);

    let mut ctrl_pressed = false;
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        editor.shape_as_needed();
        if editor.buffer().redraw() {
            let instant = Instant::now();

            let bg = editor.background_color();
            window.set(orbclient::Color::rgb(bg.r(), bg.g(), bg.b()));

            let fg = editor.foreground_color();
            editor.draw(&mut swash_cache, fg, |x, y, w, h, color| {
                window.rect(
                    line_x as i32 + x,
                    y,
                    w,
                    h,
                    orbclient::Color { data: color.0 },
                )
            });

            // Draw scrollbar
            {
                let mut start_line_opt = None;
                let mut end_line = 0;
                for run in editor.buffer().layout_runs() {
                    end_line = run.line_i;
                    if start_line_opt == None {
                        start_line_opt = Some(end_line);
                    }
                }

                let start_line = start_line_opt.unwrap_or(end_line);
                let lines = editor.buffer().lines.len();
                let start_y = (start_line * window.height() as usize) / lines;
                let end_y = (end_line * window.height() as usize) / lines;
                if end_y > start_y {
                    window.rect(
                        window.width() as i32 - line_x as i32,
                        start_y as i32,
                        line_x as u32,
                        (end_y - start_y) as u32,
                        orbclient::Color::rgba(0xFF, 0xFF, 0xFF, 0x40),
                    );
                }
            }

            window.sync();

            editor.buffer_mut().set_redraw(false);

            log::debug!("redraw: {:?}", instant.elapsed());
        }

        let mut found_event = false;
        let mut force_drag = true;
        let mut window_async = false;
        for event in window.events() {
            found_event = true;
            match event.to_option() {
                EventOption::Key(event) => match event.scancode {
                    orbclient::K_CTRL => ctrl_pressed = event.pressed,
                    orbclient::K_LEFT if event.pressed => editor.action(Action::Left),
                    orbclient::K_RIGHT if event.pressed => editor.action(Action::Right),
                    orbclient::K_UP if event.pressed => editor.action(Action::Up),
                    orbclient::K_DOWN if event.pressed => editor.action(Action::Down),
                    orbclient::K_HOME if event.pressed => editor.action(Action::Home),
                    orbclient::K_END if event.pressed => editor.action(Action::End),
                    orbclient::K_PGUP if event.pressed => editor.action(Action::PageUp),
                    orbclient::K_PGDN if event.pressed => editor.action(Action::PageDown),
                    orbclient::K_ESC if event.pressed => editor.action(Action::Escape),
                    orbclient::K_ENTER if event.pressed => editor.action(Action::Enter),
                    orbclient::K_BKSP if event.pressed => editor.action(Action::Backspace),
                    orbclient::K_DEL if event.pressed => editor.action(Action::Delete),
                    orbclient::K_0 if event.pressed && ctrl_pressed => {
                        font_size_i = font_size_default;
                        editor.buffer_mut().set_metrics(font_sizes[font_size_i]);
                    }
                    orbclient::K_MINUS if event.pressed && ctrl_pressed => {
                        if font_size_i > 0 {
                            font_size_i -= 1;
                            editor.buffer_mut().set_metrics(font_sizes[font_size_i]);
                        }
                    }
                    orbclient::K_EQUALS if event.pressed && ctrl_pressed => {
                        if font_size_i + 1 < font_sizes.len() {
                            font_size_i += 1;
                            editor.buffer_mut().set_metrics(font_sizes[font_size_i]);
                        }
                    }
                    _ => (),
                },
                EventOption::TextInput(event) if !ctrl_pressed => {
                    editor.action(Action::Insert(event.character));
                }
                EventOption::Mouse(event) => {
                    mouse_x = event.x;
                    mouse_y = event.y;
                    if mouse_left {
                        editor.action(Action::Drag {
                            x: mouse_x - line_x as i32,
                            y: mouse_y,
                        });

                        if mouse_y <= 5 {
                            editor.action(Action::Scroll { lines: -3 });
                            window_async = true;
                        } else if mouse_y + 5 >= window.height() as i32 {
                            editor.action(Action::Scroll { lines: 3 });
                            window_async = true;
                        }

                        force_drag = false;
                    }
                }
                EventOption::Button(event) => {
                    if event.left != mouse_left {
                        mouse_left = event.left;
                        if mouse_left {
                            editor.action(Action::Click {
                                x: mouse_x - line_x as i32,
                                y: mouse_y,
                            });
                        }
                        force_drag = false;
                    }
                }
                EventOption::Resize(event) => {
                    editor
                        .buffer_mut()
                        .set_size(event.width as f32 - line_x * 2.0, event.height as f32);
                }
                EventOption::Scroll(event) => {
                    editor.action(Action::Scroll {
                        lines: -event.y * 3,
                    });
                }
                EventOption::Quit(_) => return,
                _ => (),
            }
        }

        if mouse_left && force_drag {
            editor.action(Action::Drag {
                x: mouse_x - line_x as i32,
                y: mouse_y,
            });

            if mouse_y <= 5 {
                editor.action(Action::Scroll { lines: -3 });
                window_async = true;
            } else if mouse_y + 5 >= window.height() as i32 {
                editor.action(Action::Scroll { lines: 3 });
                window_async = true;
            }
        }

        if window_async != window.is_async() {
            window.set_async(window_async);
        }

        if window_async && !found_event {
            // In async mode and no event found, sleep
            thread::sleep(Duration::from_millis(5));
        }
    }
}
