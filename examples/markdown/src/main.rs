// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{Attrs, Color, Family, FontSystem, Style, SwashCache,
    TextAction, TextBuffer, TextBufferLine, TextMetrics, Weight};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, thread, time::{Duration, Instant}};

fn main() {
    env_logger::init();

    let font_system = FontSystem::new();

    let md = if let Some(arg) = env::args().nth(1) {
        fs::read_to_string(&arg).expect("failed to open file")
    } else {
        "Markdown can have **bold**, *italic*, and `code` styles".to_string()
    };
    let text = minimad::parse_text(&md);

    let mut window = Window::new_flags(
        -1,
        -1,
        1024,
        768,
        &format!("COSMIC TEXT - {}", font_system.locale),
        &[WindowFlag::Resizable],
    )
    .unwrap();

    let attrs = cosmic_text::Attrs::new();
    let mut buffer = TextBuffer::new(
        &font_system,
        attrs,
        TextMetrics::new(32, 44)
    );

    buffer.set_size(
        window.width() as i32,
        window.height() as i32
    );

    for (line_i, md_line) in text.lines.iter().enumerate() {
        while line_i >= buffer.lines.len() {
            buffer.lines.push(TextBufferLine::new(String::new(), attrs));
        }

        let composite = match md_line {
            minimad::Line::Normal(composite) => composite,
            _ => {
                //TODO: other md_line types
                continue
            }
        };

        let line = &mut buffer.lines[line_i];
        //TODO: use composite style
        for compound in composite.compounds.iter() {
            let start = line.text.len();
            line.text.push_str(compound.src);
            let end = line.text.len();
            line.attrs_list.add_span(
                start,
                end,
                attrs
                    .monospaced(compound.code)
                    .family(if compound.code { Family::Monospace } else { Family::SansSerif })
                    .style(if compound.italic { Style::Italic } else { Style::Normal })
                    .weight(if compound.bold { Weight::BOLD } else { Weight::NORMAL })
            );
            line.reset();
        }
    }


    let mut swash_cache = SwashCache::new(&font_system);

    //TODO: make window not async?
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        let bg_color = orbclient::Color::rgb(0x34, 0x34, 0x34);
        let font_color = orbclient::Color::rgb(0xFF, 0xFF, 0xFF);

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
                EventOption::Mouse(mouse) => {
                    mouse_x = mouse.x;
                    mouse_y = mouse.y;
                    if mouse_left {
                        buffer.action(TextAction::Drag { x: mouse_x, y: mouse_y });
                    }
                },
                EventOption::Button(button) => {
                    mouse_left = button.left;
                    if mouse_left {
                        buffer.action(TextAction::Click { x: mouse_x, y: mouse_y });
                    }
                },
                EventOption::Resize(resize) => {
                    buffer.set_size(resize.width as i32, resize.height as i32);
                },
                EventOption::Quit(_) => process::exit(0),
                _ => (),
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}
