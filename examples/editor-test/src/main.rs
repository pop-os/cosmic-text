// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{
    Action, BidiParagraphs, BorrowedWithFontSystem, Buffer, Color, Edit, Editor, FontSystem,
    Metrics, Motion, SwashCache,
};
use orbclient::{EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, time::Instant};
use unicode_segmentation::UnicodeSegmentation;

fn redraw(
    window: &mut Window,
    editor: &mut BorrowedWithFontSystem<Editor>,
    swash_cache: &mut SwashCache,
) {
    let bg_color = orbclient::Color::rgb(0x34, 0x34, 0x34);
    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);
    let cursor_color = Color::rgb(0xFF, 0xFF, 0xFF);
    let selection_color = Color::rgba(0xFF, 0xFF, 0xFF, 0x33);
    let selected_text_color = Color::rgb(0xF0, 0xF0, 0xFF);

    editor.shape_as_needed(true);
    if editor.redraw() {
        let instant = Instant::now();

        window.set(bg_color);

        editor.draw(
            swash_cache,
            font_color,
            cursor_color,
            selection_color,
            selected_text_color,
            |x, y, w, h, color| {
                window.rect(x, y, w, h, orbclient::Color { data: color.0 });
            },
        );

        window.sync();

        editor.set_redraw(false);

        let duration = instant.elapsed();
        log::debug!("redraw: {:?}", duration);
    }
}

fn main() {
    env_logger::init();

    let display_scale = 1.0;
    let mut font_system = FontSystem::new();

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

    let mut buffer = Buffer::new(&mut font_system, font_sizes[font_size_default]);
    buffer
        .borrow_with(&mut font_system)
        .set_size(Some(window.width() as f32), Some(window.height() as f32));

    let mut editor = Editor::new(buffer);

    let mut editor = editor.borrow_with(&mut font_system);

    let mut swash_cache = SwashCache::new();

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

    for line in BidiParagraphs::new(&text) {
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
                editor.action(Action::Motion(Motion::Previous));
                editor.action(Action::Delete);
                for c in grapheme.chars() {
                    editor.action(Action::Insert(c));
                }
                assert_eq!(
                    (cursor.line, cursor.index),
                    (editor.cursor().line, editor.cursor().index)
                );
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
            editor.action(Action::Motion(Motion::Previous));
            editor.action(Action::Delete);
            assert_eq!(cursor, editor.cursor());
        }

        // Finally, normal enter
        editor.action(Action::Enter);

        redraw(&mut window, &mut editor, &mut swash_cache);

        for event in window.events() {
            if let EventOption::Quit(_) = event.to_option() {
                process::exit(1)
            }
        }
    }

    let test_elapsed = test_start.elapsed();
    log::info!("Test completed in {:?}", test_elapsed);

    let mut wrong = 0;
    editor.with_buffer(|buffer| {
        for (line_i, line) in text.lines().enumerate() {
            let buffer_line = &buffer.lines[line_i];
            if buffer_line.text() != line {
                log::error!("line {}: {:?} != {:?}", line_i, buffer_line.text(), line);
                wrong += 1;
            }
        }
    });
    if wrong == 0 {
        log::info!("All lines matched!");
        process::exit(0);
    } else {
        log::error!("{} lines did not match!", wrong);
        process::exit(1);
    }
}
