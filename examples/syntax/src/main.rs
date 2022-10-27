// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Attrs,
    AttrsList,
    Color,
    Family,
    FontSystem,
    Style,
    SwashCache,
    TextAction,
    TextBuffer,
    TextMetrics,
    Weight
};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, time::Instant};
use syntect::highlighting::{
    FontStyle,
    Highlighter,
    HighlightState,
    RangedHighlightIterator,
    ThemeSet,
};
use syntect::parsing::{
    ParseState,
    ScopeStack,
    SyntaxSet,
};

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

    buffer.set_text(&text);
    for line in buffer.lines.iter_mut() {
        line.wrap_simple = true;
    }

    let mut bg_color = orbclient::Color::rgb(0x00, 0x00, 0x00);
    let mut font_color = orbclient::Color::rgb(0xFF, 0xFF, 0xFF);

    let now = Instant::now();

    //TODO: store newlines in buffer
    let ps = SyntaxSet::load_defaults_nonewlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-eighties.dark"];
    let highlighter = Highlighter::new(theme);

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

    log::info!("using syntax {:?}, loaded in {:?}", syntax.name, now.elapsed());

    let mut swash_cache = SwashCache::new(&font_system);

    let mut syntax_cache = Vec::<(ParseState, HighlightState)>::new();

    let mut rehighlight = true;
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        if rehighlight {
            let now = Instant::now();

            for line_i in 0..buffer.lines.len() {
                let line = &mut buffer.lines[line_i];
                if ! line.is_reset() && line_i < syntax_cache.len() {
                    continue;
                }

                let (mut parse_state, mut highlight_state) = if line_i > 0 && line_i <= syntax_cache.len() {
                    syntax_cache[line_i - 1].clone()
                } else {
                    (
                        ParseState::new(syntax),
                        HighlightState::new(&highlighter, ScopeStack::new())
                    )
                };

                let ops = parse_state.parse_line(line.text(), &ps).unwrap();
                let ranges = RangedHighlightIterator::new(
                    &mut highlight_state,
                    &ops,
                    line.text(),
                    &highlighter,
                );

                let mut attrs_list = AttrsList::new(attrs);
                for (style, _, range) in ranges {
                    attrs_list.add_span(
                        range.start,
                        range.end,
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

                if attrs_list != line.attrs_list {
                    line.attrs_list = attrs_list;
                    line.reset();
                }

                //TODO: efficiently do syntax highlighting without having to shape whole buffer
                line.shape(&font_system);

                let cache_item = (parse_state.clone(), highlight_state.clone());
                if line_i < syntax_cache.len() {
                    if syntax_cache[line_i] != cache_item {
                        syntax_cache[line_i] = cache_item;
                        if line_i + 1 < buffer.lines.len() {
                            buffer.lines[line_i + 1].reset();
                        }
                    }
                } else {
                    syntax_cache.push(cache_item);
                }
            }

            buffer.redraw = true;
            rehighlight = false;

            log::info!("Syntax highlighted in {:?}", now.elapsed());
        }

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
                    orbclient::K_ENTER if event.pressed => {
                        buffer.action(TextAction::Enter);
                        rehighlight = true;
                    },
                    orbclient::K_BKSP if event.pressed => {
                        buffer.action(TextAction::Backspace);
                        rehighlight = true;
                    },
                    orbclient::K_DEL if event.pressed => {
                        buffer.action(TextAction::Delete);
                        rehighlight = true;
                    },
                    _ => (),
                },
                EventOption::TextInput(event) => {
                    buffer.action(TextAction::Insert(event.character));
                    rehighlight = true;
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
