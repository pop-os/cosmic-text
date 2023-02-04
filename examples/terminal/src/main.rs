// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, SwashCache};
use std::cmp;
use termion::{color, cursor};

fn main() {
    // A FontSystem provides access to detected system fonts, create one per application
    let font_system = FontSystem::new();

    // A SwashCache stores rasterized glyphs, create one per application
    let mut swash_cache = SwashCache::new(&font_system);

    // Text metrics indicate the font size and line height of a buffer
    let metrics = Metrics::new(14.0, 20.0);

    // A Buffer provides shaping and layout for a UTF-8 string, create one per text widget
    let mut buffer = Buffer::new(&font_system, metrics);

    // Set a size for the text buffer, in pixels
    let width = 80u16;
    let height = 25u16;
    buffer.set_size(width as f32, height as f32);

    // Attributes indicate what font to choose
    let attrs = Attrs::new();

    // Add some text!
    buffer.set_text(" Hi, Rust! 🦀", attrs);

    // Perform shaping as desired
    buffer.shape_until_scroll();

    // Default text color (0xFF, 0xFF, 0xFF is white)
    let text_color = Color::rgb(0xFF, 0xFF, 0xFF);

    // Start on a new line
    println!();

    // Clear buffer with black background
    for _y in 0..height {
        for _x in 0..(buffer.size().0 as i32) {
            print!(
                "{} {}",
                color::Bg(color::Rgb(0, 0, 0)),
                color::Bg(color::Reset),
            );
        }
        println!();
    }

    // Go back to start
    print!("{}", cursor::Up(height));

    // Print the buffer
    let mut last_x = 0;
    let mut last_y = 0;
    buffer.draw(&mut swash_cache, text_color, |x, y, w, h, color| {
        let a = color.a();
        if a == 0 || x < 0 || y < 0 || w != 1 || h != 1 {
            // Ignore alphas of 0, or invalid x, y coordinates, or unimplemented sizes
            return;
        }

        // Scale by alpha (mimics blending with black)
        let scale = |c: u8| cmp::max(0, cmp::min(255, ((c as i32) * (a as i32)) / 255)) as u8;

        // Navigate to x coordinate
        if x > last_x {
            print!("{}", cursor::Right((x - last_x) as u16));
            last_x = x;
        } else if x < last_x {
            print!("{}", cursor::Left((last_x - x) as u16));
            last_x = x;
        }

        // Navigate to y coordinate
        if y > last_y {
            print!("{}", cursor::Down((y - last_y) as u16));
            last_y = y;
        } else if y < last_y {
            print!("{}", cursor::Up((last_y - y) as u16));
            last_y = y;
        }

        // Print a space with the expected color as the background
        print!(
            "{} {}",
            color::Bg(color::Rgb(
                scale(color.r()),
                scale(color.g()),
                scale(color.b()),
            )),
            color::Bg(color::Reset),
        );

        // Printing a space increases x coordinate
        last_x += 1;
    });

    // Skip over output
    if last_x > 0 {
        print!("{}", cursor::Left(last_x as u16));
    }
    if (last_y as u16) < height {
        print!("{}", cursor::Down(height - last_y as u16));
    }
}
