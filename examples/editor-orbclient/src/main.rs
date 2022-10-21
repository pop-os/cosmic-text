use cosmic_text::{FontSystem, TextAction, TextBuffer, TextCursor, TextLineIndex, TextMetrics};
use orbclient::{Color, EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, time::Instant};

fn main() {
    env_logger::init();

    let display_scale = match orbclient::get_display_size() {
        Ok((w, h)) => {
            log::info!("Display size: {}, {}", w, h);
            (h as i32 / 1600) + 1
        }
        Err(err) => {
            log::warn!("Failed to get display size: {}", err);
            1
        }
    };

    let font_system = FontSystem::new();

    let mut window = Window::new_flags(
        -1,
        -1,
        1024 * display_scale as u32,
        768 * display_scale as u32,
        &format!("COSMIC TEXT - {}", font_system.locale),
        &[WindowFlag::Resizable],
    )
    .unwrap();

    let font_matches = font_system.matches(|info| -> bool {
        #[cfg(feature = "mono")]
        let monospaced = true;

        #[cfg(not(feature = "mono"))]
        let monospaced = false;

        let matched = {
            info.style == fontdb::Style::Normal &&
            info.weight == fontdb::Weight::NORMAL &&
            info.stretch == fontdb::Stretch::Normal &&
            (info.monospaced == monospaced || info.post_script_name.contains("Emoji"))
        };

        if matched {
            log::debug!(
                "{:?}: family '{}' postscript name '{}' style {:?} weight {:?} stretch {:?} monospaced {:?}",
                info.id,
                info.family,
                info.post_script_name,
                info.style,
                info.weight,
                info.stretch,
                info.monospaced
            );
        }

        matched
    }).unwrap();

    let bg_color = Color::rgb(0x34, 0x34, 0x34);
    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);
    let font_sizes = [
        TextMetrics::new(10, 14).scale(display_scale), // Caption
        TextMetrics::new(14, 20).scale(display_scale), // Body
        TextMetrics::new(20, 28).scale(display_scale), // Title 4
        TextMetrics::new(24, 32).scale(display_scale), // Title 3
        TextMetrics::new(28, 36).scale(display_scale), // Title 2
        TextMetrics::new(32, 44).scale(display_scale), // Title 1
    ];
    let font_size_default = 1; // Body
    let mut font_size_i = font_size_default;

    let text = if let Some(arg) = env::args().nth(1) {
        fs::read_to_string(&arg).expect("failed to open file")
    } else {
        #[cfg(feature = "mono")]
        let default_text = include_str!("../../../sample/mono.txt");
        #[cfg(not(feature = "mono"))]
        let default_text = include_str!("../../../sample/proportional.txt");
        default_text.to_string()
    };

    let line_x = 8 * display_scale;
    let mut buffer = TextBuffer::new(
        &font_matches,
        font_sizes[font_size_i]
    );
    buffer.set_size(
        window.width() as i32 - line_x * 2,
        window.height() as i32
    );
    buffer.set_text(&text);

    let mut ctrl_pressed = false;
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        let font_size = buffer.metrics().font_size;
        let line_height = buffer.metrics().line_height;

        buffer.shape_until_cursor();

        if buffer.redraw {
            let instant = Instant::now();

            window.set(bg_color);

            buffer.draw(font_color.data, |x, y, w, h, color| {
                window.rect(line_x + x, y, w, h, Color { data: color });
            });

            /*TODO
            let mut line_y = font_size;
            let mut start_line_opt = None;
            let mut end_line = TextLineIndex::new(0);
            for (line_i, line) in buffer
                .layout_lines()
                .iter()
                .skip(buffer.scroll() as usize)
                .take(buffer.lines() as usize)
                .enumerate()
            {
                if line_y >= window.height() as i32 {
                    break;
                }

                end_line = line.line_i;
                if start_line_opt == None {
                    start_line_opt = Some(end_line);
                }

                line_y += line_height;
            }

            // Draw scrollbar
            {
                let start_line = start_line_opt.unwrap_or(end_line);
                let lines = buffer.text_lines().len();
                let start_y = (start_line.get() * window.height() as usize) / lines;
                let end_y = (end_line.get() * window.height() as usize) / lines;
                if end_y > start_y {
                    window.rect(
                        window.width() as i32 - line_x as i32,
                        start_y as i32,
                        line_x as u32,
                        (end_y - start_y) as u32,
                        Color::rgba(0xFF, 0xFF, 0xFF, 0x40),
                    );
                }
            }
            */

            window.sync();

            buffer.redraw = false;

            let duration = instant.elapsed();
            log::debug!("redraw: {:?}", duration);
        }

        for event in window.events() {
            match event.to_option() {
                EventOption::Key(event) => match event.scancode {
                    orbclient::K_CTRL => ctrl_pressed = event.pressed,
                    orbclient::K_LEFT if event.pressed => buffer.action(TextAction::Left),
                    orbclient::K_RIGHT if event.pressed => buffer.action(TextAction::Right),
                    orbclient::K_UP if event.pressed => buffer.action(TextAction::Up),
                    orbclient::K_DOWN if event.pressed => buffer.action(TextAction::Down),
                    orbclient::K_HOME if event.pressed => buffer.action(TextAction::Home),
                    orbclient::K_END if event.pressed => buffer.action(TextAction::End),
                    orbclient::K_PGUP if event.pressed => buffer.action(TextAction::PageUp),
                    orbclient::K_PGDN if event.pressed => buffer.action(TextAction::PageDown),
                    orbclient::K_ENTER if event.pressed => buffer.action(TextAction::Enter),
                    orbclient::K_BKSP if event.pressed => buffer.action(TextAction::Backspace),
                    orbclient::K_DEL if event.pressed => buffer.action(TextAction::Delete),
                    orbclient::K_0 if event.pressed && ctrl_pressed => {
                        font_size_i = font_size_default;
                        buffer.set_metrics(font_sizes[font_size_i]);
                    },
                    orbclient::K_MINUS if event.pressed && ctrl_pressed => {
                        if font_size_i > 0 {
                            font_size_i -= 1;
                            buffer.set_metrics(font_sizes[font_size_i]);
                        }
                    },
                    orbclient::K_EQUALS if event.pressed && ctrl_pressed => {
                        if font_size_i + 1 < font_sizes.len() {
                            font_size_i += 1;
                            buffer.set_metrics(font_sizes[font_size_i]);
                        }
                    },
                    orbclient::K_D if event.pressed && ctrl_pressed => {
                        // Debug by shaping whole buffer
                        log::info!("Shaping rest of buffer");
                        let instant = Instant::now();

                        buffer.shape_until(i32::max_value());

                        let elapsed = instant.elapsed();
                        log::info!("Shaped rest of buffer in {:?}", elapsed);
                    }
                    _ => (),
                },
                EventOption::TextInput(event) if !ctrl_pressed => {
                    buffer.action(TextAction::Insert(event.character));
                }
                EventOption::Mouse(event) => {
                    mouse_x = event.x;
                    mouse_y = event.y;
                    if mouse_left {
                        buffer.action(TextAction::Drag {
                            x: mouse_x - line_x,
                            y: mouse_y
                        });
                    }
                }
                EventOption::Button(event) => {
                    if event.left != mouse_left {
                        mouse_left = event.left;
                        if mouse_left {
                            buffer.action(TextAction::Click {
                                x: mouse_x - line_x,
                                y: mouse_y
                            });
                        }
                    }
                }
                EventOption::Resize(event) => {
                    buffer.set_size(
                        event.width as i32 - line_x * 2,
                        event.height as i32,
                    );
                }
                EventOption::Scroll(event) => {
                    buffer.action(TextAction::Scroll {
                        lines: -event.y * 3,
                    });
                }
                EventOption::Quit(_) => return,
                _ => (),
            }
        }
    }
}
