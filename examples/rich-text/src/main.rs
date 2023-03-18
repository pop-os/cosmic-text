// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Action, Attrs, AttrsBuilder, AttrsList, Buffer, BufferLine, Color, Edit, Editor, Family,
    FontSystem, Metrics, Spans, Style, SwashCache, Weight,
};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{
    process, thread,
    time::{Duration, Instant},
};

fn main() {
    env_logger::init();

    let mut font_system = FontSystem::new();

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
        &format!("COSMIC TEXT - {}", font_system.locale()),
        &[WindowFlag::Resizable],
    )
    .unwrap();

    let mut editor = Editor::new(Buffer::new(
        &mut font_system,
        Metrics::new(32.0, 44.0).scale(display_scale),
    ));

    let mut editor = editor.borrow_with(&mut font_system);

    editor
        .buffer_mut()
        .set_size(window.width() as f32, window.height() as f32);

    let attrs = Attrs::builder();
    let serif_attrs = attrs.clone().family(Family::Serif);
    let mono_attrs = attrs.clone().monospaced(true).family(Family::Monospace);
    let comic_attrs = attrs.clone().family(Family::Name("Comic Neue"));

    editor.buffer_mut().lines.clear();

    let lines: &[&[(&str, AttrsBuilder, Option<Color>)]] = &[
        &[
            ("B", attrs.clone().weight(Weight::BOLD), None),
            ("old ", attrs.clone(), None),
            ("I", attrs.clone().style(Style::Italic), None),
            ("talic ", attrs.clone(), None),
            ("f", attrs.clone(), None),
            ("i ", attrs.clone(), None),
            ("f", attrs.clone().weight(Weight::BOLD), None),
            ("i ", attrs.clone(), None),
            ("f", attrs.clone().style(Style::Italic), None),
            ("i ", attrs.clone(), None),
        ],
        &[
            ("Sans-Serif Normal ", attrs.clone(), None),
            ("Sans-Serif Bold ", attrs.clone().weight(Weight::BOLD), None),
            (
                "Sans-Serif Italic ",
                attrs.clone().style(Style::Italic),
                None,
            ),
            (
                "Sans-Serif Bold Italic",
                attrs.clone().weight(Weight::BOLD).style(Style::Italic),
                None,
            ),
        ],
        &[
            ("Serif Normal ", serif_attrs.clone(), None),
            (
                "Serif Bold ",
                serif_attrs.clone().weight(Weight::BOLD),
                None,
            ),
            (
                "Serif Italic ",
                serif_attrs.clone().style(Style::Italic),
                None,
            ),
            (
                "Serif Bold Italic",
                serif_attrs.weight(Weight::BOLD).style(Style::Italic),
                None,
            ),
        ],
        &[
            ("Mono Normal ", mono_attrs.clone(), None),
            ("Mono Bold ", mono_attrs.clone().weight(Weight::BOLD), None),
            (
                "Mono Italic ",
                mono_attrs.clone().style(Style::Italic),
                None,
            ),
            (
                "Mono Bold Italic",
                mono_attrs.weight(Weight::BOLD).style(Style::Italic),
                None,
            ),
        ],
        &[
            ("Comic Normal ", comic_attrs.clone(), None),
            (
                "Comic Bold ",
                comic_attrs.clone().weight(Weight::BOLD),
                None,
            ),
            (
                "Comic Italic ",
                comic_attrs.clone().style(Style::Italic),
                None,
            ),
            (
                "Comic Bold Italic",
                comic_attrs.weight(Weight::BOLD).style(Style::Italic),
                None,
            ),
        ],
        &[
            ("R", attrs.clone(), Some(Color::rgb(0xFF, 0x00, 0x00))),
            ("A", attrs.clone(), Some(Color::rgb(0xFF, 0x7F, 0x00))),
            ("I", attrs.clone(), Some(Color::rgb(0xFF, 0xFF, 0x00))),
            ("N", attrs.clone(), Some(Color::rgb(0x00, 0xFF, 0x00))),
            ("B", attrs.clone(), Some(Color::rgb(0x00, 0x00, 0xFF))),
            ("O", attrs.clone(), Some(Color::rgb(0x4B, 0x00, 0x82))),
            ("W ", attrs.clone(), Some(Color::rgb(0x94, 0x00, 0xD3))),
            ("Red ", attrs.clone(), Some(Color::rgb(0xFF, 0x00, 0x00))),
            ("Orange ", attrs.clone(), Some(Color::rgb(0xFF, 0x7F, 0x00))),
            ("Yellow ", attrs.clone(), Some(Color::rgb(0xFF, 0xFF, 0x00))),
            ("Green ", attrs.clone(), Some(Color::rgb(0x00, 0xFF, 0x00))),
            ("Blue ", attrs.clone(), Some(Color::rgb(0x00, 0x00, 0xFF))),
            ("Indigo ", attrs.clone(), Some(Color::rgb(0x4B, 0x00, 0x82))),
            ("Violet ", attrs.clone(), Some(Color::rgb(0x94, 0x00, 0xD3))),
            ("U", attrs.clone(), Some(Color::rgb(0x94, 0x00, 0xD3))),
            ("N", attrs.clone(), Some(Color::rgb(0x4B, 0x00, 0x82))),
            ("I", attrs.clone(), Some(Color::rgb(0x00, 0x00, 0xFF))),
            ("C", attrs.clone(), Some(Color::rgb(0x00, 0xFF, 0x00))),
            ("O", attrs.clone(), Some(Color::rgb(0xFF, 0xFF, 0x00))),
            ("R", attrs.clone(), Some(Color::rgb(0xFF, 0x7F, 0x00))),
            ("N", attrs.clone(), Some(Color::rgb(0xFF, 0x00, 0x00))),
        ],
        &[(
            "生活,삶,जिंदगी 😀 FPS",
            attrs.clone(),
            Some(Color::rgb(0xFF, 0x00, 0x00)),
        )],
    ];
    for line in lines {
        let mut line_text = String::new();
        let mut attrs_list = AttrsList::new(attrs.clone().build());
        let mut color_spans = Spans::<Color>::default();
        for (text, attrs, color) in line.iter() {
            let start = line_text.len();
            line_text.push_str(text);
            let end = line_text.len();
            attrs_list.add_span(start..end, attrs.clone().build());
            if let Some(color) = color {
                color_spans.add(start..end, *color);
            }
        }
        editor
            .buffer_mut()
            .lines
            .push(BufferLine::new(line_text, attrs_list, color_spans));
    }

    let mut swash_cache = SwashCache::new();

    //TODO: make window not async?
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        let bg_color = orbclient::Color::rgb(0x34, 0x34, 0x34);
        let font_color = Color::rgb(0xFF, 0xFF, 0xFF);

        editor.shape_as_needed();
        if editor.buffer().redraw() {
            let instant = Instant::now();

            window.set(bg_color);

            editor.draw(&mut swash_cache, font_color, |x, y, w, h, color| {
                window.rect(x, y, w, h, orbclient::Color { data: color.0 });
            });

            window.sync();

            editor.buffer_mut().set_redraw(false);

            let duration = instant.elapsed();
            log::debug!("redraw: {:?}", duration);
        }

        for event in window.events() {
            match event.to_option() {
                EventOption::Key(event) => match event.scancode {
                    orbclient::K_LEFT if event.pressed => editor.action(Action::Left),
                    orbclient::K_RIGHT if event.pressed => editor.action(Action::Right),
                    orbclient::K_UP if event.pressed => editor.action(Action::Up),
                    orbclient::K_DOWN if event.pressed => editor.action(Action::Down),
                    orbclient::K_HOME if event.pressed => editor.action(Action::Home),
                    orbclient::K_END if event.pressed => editor.action(Action::End),
                    orbclient::K_PGUP if event.pressed => editor.action(Action::PageUp),
                    orbclient::K_PGDN if event.pressed => editor.action(Action::PageDown),
                    orbclient::K_ENTER if event.pressed => editor.action(Action::Enter),
                    orbclient::K_BKSP if event.pressed => editor.action(Action::Backspace),
                    orbclient::K_DEL if event.pressed => editor.action(Action::Delete),
                    _ => (),
                },
                EventOption::TextInput(event) => editor.action(Action::Insert(event.character)),
                EventOption::Mouse(mouse) => {
                    mouse_x = mouse.x;
                    mouse_y = mouse.y;
                    if mouse_left {
                        editor.action(Action::Drag {
                            x: mouse_x,
                            y: mouse_y,
                        });
                    }
                }
                EventOption::Button(button) => {
                    mouse_left = button.left;
                    if mouse_left {
                        editor.action(Action::Click {
                            x: mouse_x,
                            y: mouse_y,
                        });
                    }
                }
                EventOption::Resize(resize) => {
                    editor
                        .buffer_mut()
                        .set_size(resize.width as f32, resize.height as f32);
                }
                EventOption::Quit(_) => process::exit(0),
                _ => (),
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}
