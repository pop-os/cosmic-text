// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{FontSystem, TextAction, TextBuffer, TextMetrics};
use orbclient::{Color, EventOption, Renderer, Window, WindowFlag};
use std::{env, fs, process, thread, time::{Duration, Instant}};

fn redraw(window: &mut Window, buffer: &mut TextBuffer<'_>) {
    let bg_color = Color::rgb(0x34, 0x34, 0x34);
    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);

    buffer.shape_until_cursor();

    if buffer.redraw {
        let instant = Instant::now();

        window.set(bg_color);

        buffer.draw(font_color.data, |x, y, w, h, color| {
            window.rect(x, y, w, h, Color { data: color });
        });

        window.sync();

        buffer.redraw = false;

        let duration = instant.elapsed();
        log::debug!("redraw: {:?}", duration);
    }
}

fn main() {
    env_logger::init();

    let display_scale = 1;
    let font_system = FontSystem::new();

    let mut window = Window::new_flags(
        -1,
        -1,
        1920,
        1080,
        &format!("COSMIC TEXT - {}", font_system.locale),
        &[WindowFlag::Async],
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

    let font_sizes = [
        TextMetrics::new(10, 14).scale(display_scale), // Caption
        TextMetrics::new(14, 20).scale(display_scale), // Body
        TextMetrics::new(20, 28).scale(display_scale), // Title 4
        TextMetrics::new(24, 32).scale(display_scale), // Title 3
        TextMetrics::new(28, 36).scale(display_scale), // Title 2
        TextMetrics::new(32, 44).scale(display_scale), // Title 1
    ];
    let font_size_default = 1; // Body

    let mut buffer = TextBuffer::new(
        &font_matches,
        font_sizes[font_size_default]
    );
    buffer.set_size(
        window.width() as i32,
        window.height() as i32
    );

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

        for c in line.chars() {
            log::trace!("Insert {:?}", c);

            // Test backspace of character
            {
                let cursor = buffer.cursor();
                buffer.action(TextAction::Insert(c));
                buffer.action(TextAction::Backspace);
                assert_eq!(cursor, buffer.cursor());
            }

            // Test delete of character
            {
                let cursor = buffer.cursor();
                buffer.action(TextAction::Insert(c));
                buffer.action(TextAction::Previous);
                buffer.action(TextAction::Delete);
                assert_eq!(cursor, buffer.cursor());
            }

            // Finally, normal insert of character
            buffer.action(TextAction::Insert(c));
        }

        // Test backspace of newline
        {
            let cursor = buffer.cursor();
            buffer.action(TextAction::Enter);
            buffer.action(TextAction::Backspace);
            assert_eq!(cursor, buffer.cursor());
        }

        // Test delete of newline
        {
            let cursor = buffer.cursor();
            buffer.action(TextAction::Enter);
            buffer.action(TextAction::Previous);
            buffer.action(TextAction::Delete);
            assert_eq!(cursor, buffer.cursor());
        }

        // Finally, normal enter
        buffer.action(TextAction::Enter);

        redraw(&mut window, &mut buffer);

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
    let buffer_lines = buffer.text_lines();
    for (line_i, line) in text.lines().enumerate() {
        let buffer_line = &buffer_lines[line_i];
        if buffer_line.text() != line {
            log::error!("line {}: {:?} != {:?}", line_i, buffer_line.text(), line);
            wrong += 1;
        }
    }
    if wrong == 0 {
        log::info!("All lines matched!");
    } else {
        log::error!("{} lines did not match!", wrong);
    }

    //TODO: make window not async?
    loop {
        for event in window.events() {
            match event.to_option() {
                EventOption::Quit(_) => process::exit(0),
                _ => (),
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}
