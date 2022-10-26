// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{Attrs, Color, Family, FontSystem, Style, SwashCache,
    TextAction, TextBuffer, TextBufferLine, TextMetrics, Weight};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, thread, time::{Duration, Instant}};

fn main() {
    env_logger::init();

    let font_system = FontSystem::new();

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

    let serif_attrs = attrs.family(Family::Serif);
    let mono_attrs = attrs.monospaced(true).family(Family::Monospace);
    let comic_attrs = attrs.family(Family::Name("Comic Neue"));

    let mut line_i = 0;
    for &(text, attrs) in &[
        ("B", attrs.weight(Weight::BOLD)),
        ("old ", attrs),
        ("I", attrs.style(Style::Italic)),
        ("talic ", attrs),
        ("f", attrs),
        ("i ", attrs),
        ("f", attrs.weight(Weight::BOLD)),
        ("i ", attrs),
        ("f", attrs.style(Style::Italic)),
        ("i ", attrs),
        ("\n", attrs),
        ("Sans-Serif Normal ", attrs),
        ("Sans-Serif Bold ", attrs.weight(Weight::BOLD)),
        ("Sans-Serif Italic ", attrs.style(Style::Italic)),
        ("Sans-Serif Bold Italic", attrs.weight(Weight::BOLD).style(Style::Italic)),
        ("\n", attrs),
        ("Serif Normal ", serif_attrs),
        ("Serif Bold ", serif_attrs.weight(Weight::BOLD)),
        ("Serif Italic ", serif_attrs.style(Style::Italic)),
        ("Serif Bold Italic", serif_attrs.weight(Weight::BOLD).style(Style::Italic)),
        ("\n", attrs),
        ("Mono Normal ", mono_attrs),
        ("Mono Bold ", mono_attrs.weight(Weight::BOLD)),
        ("Mono Italic ", mono_attrs.style(Style::Italic)),
        ("Mono Bold Italic", mono_attrs.weight(Weight::BOLD).style(Style::Italic)),
        ("\n", attrs),
        ("Comic Normal ", comic_attrs),
        ("Comic Bold ", comic_attrs.weight(Weight::BOLD)),
        ("Comic Italic ", comic_attrs.style(Style::Italic)),
        ("Comic Bold Italic", comic_attrs.weight(Weight::BOLD).style(Style::Italic)),
        ("\n", attrs),
        ("R", attrs.color(Color::rgb(0xFF, 0x00, 0x00))),
        ("A", attrs.color(Color::rgb(0xFF, 0x7F, 0x00))),
        ("I", attrs.color(Color::rgb(0xFF, 0xFF, 0x00))),
        ("N", attrs.color(Color::rgb(0x00, 0xFF, 0x00))),
        ("B", attrs.color(Color::rgb(0x00, 0x00, 0xFF))),
        ("O", attrs.color(Color::rgb(0x4B, 0x00, 0x82))),
        ("W ", attrs.color(Color::rgb(0x94, 0x00, 0xD3))),
        ("Red ", attrs.color(Color::rgb(0xFF, 0x00, 0x00))),
        ("Orange ", attrs.color(Color::rgb(0xFF, 0x7F, 0x00))),
        ("Yellow ", attrs.color(Color::rgb(0xFF, 0xFF, 0x00))),
        ("Green ", attrs.color(Color::rgb(0x00, 0xFF, 0x00))),
        ("Blue ", attrs.color(Color::rgb(0x00, 0x00, 0xFF))),
        ("Indigo ", attrs.color(Color::rgb(0x4B, 0x00, 0x82))),
        ("Violet ", attrs.color(Color::rgb(0x94, 0x00, 0xD3))),
        ("U", attrs.color(Color::rgb(0x94, 0x00, 0xD3))),
        ("N", attrs.color(Color::rgb(0x4B, 0x00, 0x82))),
        ("I", attrs.color(Color::rgb(0x00, 0x00, 0xFF))),
        ("C", attrs.color(Color::rgb(0x00, 0xFF, 0x00))),
        ("O", attrs.color(Color::rgb(0xFF, 0xFF, 0x00))),
        ("R", attrs.color(Color::rgb(0xFF, 0x7F, 0x00))),
        ("N", attrs.color(Color::rgb(0xFF, 0x00, 0x00))),
    ] {
        if text == "\n" {
            line_i += 1;
            while line_i >= buffer.lines.len() {
                buffer.lines.push(TextBufferLine::new(String::new(), attrs));
            }
            continue;
        }

        let line = &mut buffer.lines[line_i];
        let start = line.text.len();
        line.text.push_str(text);
        let end = line.text.len();
        line.attrs_list.add_span(start, end, attrs);
        line.reset();
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
