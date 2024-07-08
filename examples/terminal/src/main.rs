// SPDX-License-Identifier: MIT OR Apache-2.0

//! Run this example with `cargo run --package terminal`
//! or `cargo run --package terminal -- "my own text"`

use colored::Colorize;
use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use std::fmt::Write;

fn main() {
    // A FontSystem provides access to detected system fonts, create one per application
    let mut font_system = FontSystem::new();

    // A SwashCache stores rasterized glyphs, create one per application
    let mut swash_cache = SwashCache::new();

    // Text metrics indicate the font size and line height of a buffer
    const FONT_SIZE: f32 = 14.0;
    const LINE_HEIGHT: f32 = FONT_SIZE * 1.2;
    let metrics = Metrics::new(FONT_SIZE, LINE_HEIGHT);

    // A Buffer provides shaping and layout for a UTF-8 string, create one per text widget
    let mut buffer = Buffer::new(&mut font_system, metrics);

    let mut buffer = buffer.borrow_with(&mut font_system);

    // Set a size for the text buffer, in pixels
    let width = 80.0;
    // The height is unbounded
    buffer.set_size(Some(width), None);

    // Attributes indicate what font to choose
    let attrs = Attrs::new();

    // Add some text!
    let text = std::env::args()
        .nth(1)
        .unwrap_or(" Hi, Rust! 🦀 ".to_string());
    buffer.set_text(&text, attrs, Shaping::Advanced);

    // Perform shaping as desired
    buffer.shape_until_scroll(true);

    // Default text color (0xFF, 0xFF, 0xFF is white)
    const TEXT_COLOR: Color = Color::rgb(0xFF, 0xFF, 0xFF);

    // Set up the canvas
    let height = LINE_HEIGHT * buffer.layout_runs().count() as f32;
    let mut canvas = vec![vec![None; width as usize]; height as usize];

    // Draw to the canvas
    buffer.draw(&mut swash_cache, TEXT_COLOR, |x, y, w, h, color| {
        let a = color.a();
        if a == 0 || x < 0 || x >= width as i32 || y < 0 || y >= height as i32 || w != 1 || h != 1 {
            // Ignore alphas of 0, or invalid x, y coordinates, or unimplemented sizes
            return;
        }

        // Scale by alpha (mimics blending with black)
        let scale = |c: u8| (c as i32 * a as i32 / 255).clamp(0, 255) as u8;

        let r = scale(color.r());
        let g = scale(color.g());
        let b = scale(color.b());
        canvas[y as usize][x as usize] = Some((r, g, b));
    });

    // Render the canvas
    let mut output = String::new();

    for row in canvas {
        for pixel in row {
            let (r, g, b) = pixel.unwrap_or((0, 0, 0));
            write!(&mut output, "{}", "  ".on_truecolor(r, g, b)).ok();
        }
        writeln!(&mut output).ok();
    }

    print!("{}", output);
}
