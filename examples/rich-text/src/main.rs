// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Action, Attrs, Buffer, CacheKeyFlags, Color, Edit, Editor, Family, FontSystem, Metrics, Motion,
    Shaping, Style, SwashCache, Weight,
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

    let mut editor = Editor::new(Buffer::new_empty(
        Metrics::new(32.0, 44.0).scale(display_scale),
    ));

    let mut editor = editor.borrow_with(&mut font_system);

    editor.with_buffer_mut(|buffer| buffer.set_size(window.width() as f32, window.height() as f32));

    let attrs = Attrs::new();
    let serif_attrs = attrs.family(Family::Serif);
    let mono_attrs = attrs.family(Family::Monospace);
    let comic_attrs = attrs.family(Family::Name("Comic Neue"));

    let spans: &[(&str, Attrs)] = &[
        ("B", attrs.weight(Weight::BOLD)),
        ("old ", attrs),
        ("I", attrs.style(Style::Italic)),
        ("talic ", attrs),
        ("f", attrs),
        ("i ", attrs),
        ("f", attrs.weight(Weight::BOLD)),
        ("i ", attrs),
        ("f", attrs.style(Style::Italic)),
        ("i \n", attrs),
        ("Sans-Serif Normal ", attrs),
        ("Sans-Serif Bold ", attrs.weight(Weight::BOLD)),
        ("Sans-Serif Italic ", attrs.style(Style::Italic)),
        (
            "Sans-Serif Fake Italic ",
            attrs.cache_key_flags(CacheKeyFlags::FAKE_ITALIC),
        ),
        (
            "Sans-Serif Bold Italic\n",
            attrs.weight(Weight::BOLD).style(Style::Italic),
        ),
        ("Serif Normal ", serif_attrs),
        ("Serif Bold ", serif_attrs.weight(Weight::BOLD)),
        ("Serif Italic ", serif_attrs.style(Style::Italic)),
        (
            "Serif Bold Italic\n",
            serif_attrs.weight(Weight::BOLD).style(Style::Italic),
        ),
        ("Mono Normal ", mono_attrs),
        ("Mono Bold ", mono_attrs.weight(Weight::BOLD)),
        ("Mono Italic ", mono_attrs.style(Style::Italic)),
        (
            "Mono Bold Italic\n",
            mono_attrs.weight(Weight::BOLD).style(Style::Italic),
        ),
        ("Comic Normal ", comic_attrs),
        ("Comic Bold ", comic_attrs.weight(Weight::BOLD)),
        ("Comic Italic ", comic_attrs.style(Style::Italic)),
        (
            "Comic Bold Italic\n",
            comic_attrs.weight(Weight::BOLD).style(Style::Italic),
        ),
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
        ("N\n", attrs.color(Color::rgb(0xFF, 0x00, 0x00))),
        (
            "生活,삶,जिंदगी 😀 FPS\n",
            attrs.color(Color::rgb(0xFF, 0x00, 0x00)),
        ),
    ];

    editor.with_buffer_mut(|buffer| {
        buffer.set_rich_text(spans.iter().copied(), attrs, Shaping::Advanced)
    });

    let mut swash_cache = SwashCache::new();

    //TODO: make window not async?
    let mut mouse_x = -1;
    let mut mouse_y = -1;
    let mut mouse_left = false;
    loop {
        let bg_color = orbclient::Color::rgb(0x34, 0x34, 0x34);
        let font_color = Color::rgb(0xFF, 0xFF, 0xFF);
        let cursor_color = Color::rgb(0xFF, 0xFF, 0xFF);
        let selection_color = Color::rgba(0xFF, 0xFF, 0xFF, 0x33);

        editor.shape_as_needed(true);
        if editor.redraw() {
            let instant = Instant::now();

            window.set(bg_color);

            editor.draw(
                &mut swash_cache,
                font_color,
                cursor_color,
                selection_color,
                |x, y, w, h, color| {
                    window.rect(x, y, w, h, orbclient::Color { data: color.0 });
                },
            );

            window.sync();

            editor.set_redraw(false);

            let duration = instant.elapsed();
            log::debug!("redraw: {:?}", duration);
        }

        for event in window.events() {
            match event.to_option() {
                EventOption::Key(event) => match event.scancode {
                    orbclient::K_LEFT if event.pressed => {
                        editor.action(Action::Motion(Motion::Left))
                    }
                    orbclient::K_RIGHT if event.pressed => {
                        editor.action(Action::Motion(Motion::Right))
                    }
                    orbclient::K_UP if event.pressed => editor.action(Action::Motion(Motion::Up)),
                    orbclient::K_DOWN if event.pressed => {
                        editor.action(Action::Motion(Motion::Down))
                    }
                    orbclient::K_HOME if event.pressed => {
                        editor.action(Action::Motion(Motion::Home))
                    }
                    orbclient::K_END if event.pressed => editor.action(Action::Motion(Motion::End)),
                    orbclient::K_PGUP if event.pressed => {
                        editor.action(Action::Motion(Motion::PageUp))
                    }
                    orbclient::K_PGDN if event.pressed => {
                        editor.action(Action::Motion(Motion::PageDown))
                    }
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
                    editor.with_buffer_mut(|buffer| {
                        buffer.set_size(resize.width as f32, resize.height as f32)
                    });
                }
                EventOption::Quit(_) => process::exit(0),
                _ => (),
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}
