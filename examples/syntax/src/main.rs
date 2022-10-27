// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{Attrs, AttrsList, Color, Family, FontSystem, Style, SwashCache,
    TextAction, TextBuffer, TextBufferLine, TextMetrics, Weight};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, time::Instant};

fn main() {
    env_logger::init();

    let font_system = FontSystem::new();

    let (path, text) = if let Some(arg) = env::args().nth(1) {
        (
            arg.clone(),
            fs::read_to_string(&arg).expect("failed to open file")
        )
    } else {
        (
            String::new(),
            String::new()
        )
    };

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

    let mut window = Window::new_flags(
        -1,
        -1,
        1024 * display_scale as u32,
        768 * display_scale as u32,
        &format!("COSMIC TEXT - {}", font_system.locale),
        &[WindowFlag::Resizable],
    )
    .unwrap();

    let attrs = Attrs::new()
        .monospaced(true)
        .family(Family::Monospace);
    let mut buffer = TextBuffer::new(
        &font_system,
        attrs,
        TextMetrics::new(14, 20).scale(display_scale)
    );

    buffer.set_size(
        window.width() as i32,
        window.height() as i32
    );

    let mut bg_color = orbclient::Color::rgb(0x00, 0x00, 0x00);
    let mut font_color = orbclient::Color::rgb(0xFF, 0xFF, 0xFF);
    {
        use syntect::easy::HighlightLines;
        use syntect::parsing::SyntaxSet;
        use syntect::highlighting::{ThemeSet, FontStyle, Style as SyntectStyle};
        use syntect::util::LinesWithEndings;

        let now = Instant::now();

        // Load these once at the start of your program
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = &ts.themes["base16-eighties.dark"];

        if let Some(background) = theme.settings.background {
            bg_color = orbclient::Color::rgba(
                background.r,
                background.g,
                background.b,
                background.a,
            );
        }

        if let Some(foreground) = theme.settings.foreground {
            font_color = orbclient::Color::rgba(
                foreground.r,
                foreground.g,
                foreground.b,
                foreground.a,
            );
        }

        let syntax = match ps.find_syntax_for_file(&path) {
            Ok(Some(some)) => some,
            Ok(None) => {
                log::warn!("no syntax found for {:?}", path);
                ps.find_syntax_plain_text()
            }
            Err(err) => {
                log::warn!("failed to determine syntax for {:?}: {:?}", path, err);
                ps.find_syntax_plain_text()
            }
        };
        log::info!("using syntax {:?}", syntax.name);

        buffer.lines.clear();

        let mut h = HighlightLines::new(syntax, &theme);
        for highlight_line in LinesWithEndings::from(&text) { // LinesWithEndings enables use of newlines mode
            let ranges: Vec<(SyntectStyle, &str)> = h.highlight_line(highlight_line, &ps).unwrap();

            let mut line_text = String::new();
            let mut attrs_list = AttrsList::new(attrs);
            for (style, string) in ranges.iter() {
                let string_trim = match string.lines().next() {
                    Some(some) => some,
                    None => continue,
                };

                let start = line_text.len();
                line_text.push_str(string_trim);
                let end = line_text.len();
                attrs_list.add_span(
                    start,
                    end,
                    attrs
                        .color(Color::rgba(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                            style.foreground.a,
                        ))
                        //TODO: background
                        .style(if style.font_style.contains(FontStyle::ITALIC) {
                            Style::Italic
                        } else {
                            Style::Normal
                        })
                        .weight(if style.font_style.contains(FontStyle::BOLD) {
                            Weight::BOLD
                        } else {
                            Weight::NORMAL
                        })
                        //TODO: underline
                );
            }

            buffer.lines.push(TextBufferLine::new(line_text, attrs_list));
        }

        let elapsed = now.elapsed();
        log::info!("Syntax highlighted in {:?}", elapsed);
    }

    let mut swash_cache = SwashCache::new(&font_system);

    //TODO: make window not async?
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        if buffer.cursor_moved {
            buffer.shape_until_cursor();
            buffer.cursor_moved = false;
        } else {
            buffer.shape_until_scroll();
        }

        if buffer.redraw {
            let instant = Instant::now();

            window.set(bg_color);

            buffer.draw(&mut swash_cache, font_color.data, |x, y, w, h, color| {
                window.rect(x, y, w, h, orbclient::Color { data: color });
            });

            window.sync();

            buffer.redraw = false;

            let duration = instant.elapsed();
            log::debug!("redraw: {:?}", duration);
        }

        for event in window.events() {
            match event.to_option() {
                EventOption::Key(event) => match event.scancode {
                    orbclient::K_LEFT if event.pressed => buffer.action(TextAction::Left),
                    orbclient::K_RIGHT if event.pressed => buffer.action(TextAction::Right),
                    orbclient::K_UP if event.pressed => buffer.action(TextAction::Up),
                    orbclient::K_DOWN if event.pressed => buffer.action(TextAction::Down),
                    orbclient::K_HOME if event.pressed => buffer.action(TextAction::Home),
                    orbclient::K_END if event.pressed => buffer.action(TextAction::End),
                    orbclient::K_PGUP if event.pressed => buffer.action(TextAction::PageUp),
                    orbclient::K_PGDN if event.pressed => buffer.action(TextAction::PageDown),
                    _ => (),
                },
                EventOption::Mouse(event) => {
                    mouse_x = event.x;
                    mouse_y = event.y;
                    if mouse_left {
                        buffer.action(TextAction::Drag { x: mouse_x, y: mouse_y });
                    }
                },
                EventOption::Button(event) => {
                    if event.left != mouse_left {
                        mouse_left = event.left;
                        if mouse_left {
                            buffer.action(TextAction::Click { x: mouse_x, y: mouse_y });
                        }
                    }
                },
                EventOption::Resize(event) => {
                    buffer.set_size(event.width as i32, event.height as i32);
                    buffer.redraw = true;
                },
                EventOption::Scroll(event) => {
                    buffer.action(TextAction::Scroll {
                        lines: -event.y * 3,
                    });
                }
                EventOption::Quit(_) => process::exit(0),
                _ => (),
            }
        }
    }
}
