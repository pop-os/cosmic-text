// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{Action, Buffer, Color, Edit, Editor, FontSystem, Metrics, SwashCache};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, time::Instant};
use unicode_segmentation::UnicodeSegmentation;

fn redraw(window: &mut Window, editor: &mut Editor<'_>, swash_cache: &mut SwashCache) {
    let bg_color = orbclient::Color::rgb(0x34, 0x34, 0x34);
    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);

    editor.shape_as_needed();
    if editor.buffer().redraw() {
        let instant = Instant::now();

        window.set(bg_color);

        editor.draw(swash_cache, font_color, |x, y, w, h, color| {
            window.rect(x, y, w, h, orbclient::Color { data: color.0 });
        });

        window.sync();

        editor.buffer_mut().set_redraw(false);

        let duration = instant.elapsed();
        log::debug!("redraw: {:?}", duration);
    }
}

fn main() {
    env_logger::init();

    let display_scale = 1.0;
    let font_system = FontSystem::new();

    let mut window = Window::new_flags(
        -1,
        -1,
        1024,
        768,
        &format!("COSMIC TEXT - {}", font_system.locale()),
        &[WindowFlag::Async],
    )
    .unwrap();

    let font_sizes = [
        Metrics::new(10.0, 14.0).scale(display_scale), // Caption
        Metrics::new(14.0, 20.0).scale(display_scale), // Body
        Metrics::new(20.0, 28.0).scale(display_scale), // Title 4
        Metrics::new(24.0, 32.0).scale(display_scale), // Title 3
        Metrics::new(28.0, 36.0).scale(display_scale), // Title 2
        Metrics::new(32.0, 44.0).scale(display_scale), // Title 1
    ];
    let font_size_default = 1; // Body

    let mut buffer = Buffer::new(&font_system, font_sizes[font_size_default]);
    buffer.set_size(window.width() as f32, window.height() as f32);

    let mut editor = Editor::new(buffer);

    let mut swash_cache = SwashCache::new(&font_system);

    let text = if let Some(arg) = env::args().nth(1) {
        fs::read_to_string(&arg).expect("failed to open file")
    } else {
        #[cfg(feature = "mono")]
        let default_text = include_str!("../../../sample/mono.txt");
        #[cfg(not(feature = "mono"))]
        let default_text = include_str!("../../../sample/proportional.txt");
        default_text.to_string()
    };

    let test_start = Instant::now();

    //TODO: support bidi
    for line in text.lines() {
        log::debug!("Line {:?}", line);

        for grapheme in line.graphemes(true) {
            for c in grapheme.chars() {
                log::trace!("Insert {:?}", c);

                // Test backspace of character
                {
                    let cursor = editor.cursor();
                    editor.action(Action::Insert(c));
                    editor.action(Action::Backspace);
                    assert_eq!(cursor, editor.cursor());
                }

                // Finally, normal insert of character
                editor.action(Action::Insert(c));
            }

            // Test delete of EGC
            {
                let cursor = editor.cursor();
                editor.action(Action::Previous);
                editor.action(Action::Delete);
                for c in grapheme.chars() {
                    editor.action(Action::Insert(c));
                }
                assert_eq!(cursor, editor.cursor());
            }
        }

        // Test backspace of newline
        {
            let cursor = editor.cursor();
            editor.action(Action::Enter);
            editor.action(Action::Backspace);
            assert_eq!(cursor, editor.cursor());
        }

        // Test delete of newline
        {
            let cursor = editor.cursor();
            editor.action(Action::Enter);
            editor.action(Action::Previous);
            editor.action(Action::Delete);
            assert_eq!(cursor, editor.cursor());
        }

        // Finally, normal enter
        editor.action(Action::Enter);

        redraw(&mut window, &mut editor, &mut swash_cache);

        for event in window.events() {
            match event.to_option() {
                EventOption::Quit(_) => process::exit(1),
                _ => (),
            }
        }
    }

    let test_elapsed = test_start.elapsed();
    log::info!("Test completed in {:?}", test_elapsed);

    let mut wrong = 0;
    for (line_i, line) in text.lines().enumerate() {
        let buffer_line = &editor.buffer().lines[line_i];
        if buffer_line.text() != line {
            log::error!("line {}: {:?} != {:?}", line_i, buffer_line.text(), line);
            wrong += 1;
        }
    }
    if wrong == 0 {
        log::info!("All lines matched!");
        process::exit(0);
    } else {
        log::error!("{} lines did not match!", wrong);
        process::exit(1);
    }
}
