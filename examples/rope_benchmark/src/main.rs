use std::env;
use std::fs;
use std::time::Instant;

use cosmic_text::{Attrs, Buffer, FontSystem, LoadedBuffer, Metrics, RopeBuffer, Shaping};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        eprintln!("");
        eprintln!("This benchmark compares Vec<BufferLine> vs RopeBuffer for large files.");
        eprintln!("");
        eprintln!("To generate a test file:");
        eprintln!("  yes 'The quick brown fox jumps over the lazy dog.' | head -n 2000000 > /tmp/large_test.txt");
        std::process::exit(1);
    }

    let file_path = &args[1];

    println!("Loading file: {}", file_path);
    let start = Instant::now();
    let content = fs::read_to_string(file_path).expect("Failed to read file");
    let load_time = start.elapsed();

    let file_size_mb = content.len() as f64 / (1024.0 * 1024.0);
    let line_count = content.lines().count();

    println!("File size: {:.2} MB", file_size_mb);
    println!("Line count: {}", line_count);
    println!("File read time: {:?}", load_time);
    println!("");

    // Initialize font system
    println!("Initializing font system...");
    let start = Instant::now();
    let mut font_system = FontSystem::new();
    println!("Font system init: {:?}", start.elapsed());
    println!("");

    let metrics = Metrics::new(14.0, 20.0);
    let attrs = Attrs::new();

    // Test LoadedBuffer (auto-selects based on size)
    println!("=== LoadedBuffer (auto-select) ===");
    let start = Instant::now();
    let loaded = LoadedBuffer::from_text_auto(
        &mut font_system,
        &content,
        &attrs,
        metrics,
        Shaping::Advanced,
    );
    let total_time = start.elapsed();

    println!("  Backend: {}", if loaded.is_rope() { "RopeBuffer" } else { "Standard Buffer" });
    println!("  Line count: {}", loaded.line_count());
    println!("  TOTAL time: {:?}", total_time);
    println!("");

    // Test RopeBuffer directly
    println!("=== RopeBuffer (rope-based) ===");
    let start = Instant::now();
    let mut rope_buffer = RopeBuffer::new_empty(metrics);
    let create_time = start.elapsed();

    let start = Instant::now();
    rope_buffer.set_text(&mut font_system, &content, &attrs, Shaping::Advanced, None);
    let set_text_time = start.elapsed();

    println!("  Create buffer: {:?}", create_time);
    println!("  set_text():    {:?}", set_text_time);
    println!("  Line count:    {}", rope_buffer.line_count());
    println!("  TOTAL:         {:?}", create_time + set_text_time);
    println!("");

    // Memory estimate for RopeBuffer
    let rope_mem_estimate = content.len() as f64 * 2.0 / (1024.0 * 1024.0);
    println!("  Estimated memory: ~{:.1} MB (rope overhead ~2x text)", rope_mem_estimate);
    println!("");

    // For comparison, show what standard buffer would be like
    // (only run for smaller files to avoid timeout)
    if content.len() < 10_000_000 {
        println!("=== Standard Buffer (Vec<BufferLine>) ===");
        let start = Instant::now();
        let mut buffer = Buffer::new_empty(metrics);
        let create_time = start.elapsed();

        let start = Instant::now();
        buffer.set_text(&mut font_system, &content, &attrs, Shaping::Advanced, None);
        let set_text_time = start.elapsed();

        println!("  Create buffer: {:?}", create_time);
        println!("  set_text():    {:?}", set_text_time);
        println!("  Line count:    {}", buffer.line_count());
        println!("  TOTAL:         {:?}", create_time + set_text_time);

        let vec_mem_estimate = (buffer.line_count() as f64 * 150.0 + content.len() as f64) / (1024.0 * 1024.0);
        println!("  Estimated memory: ~{:.1} MB", vec_mem_estimate);
    } else {
        println!("=== Standard Buffer (Vec<BufferLine>) ===");
        println!("  SKIPPED - file too large (would timeout/OOM)");
        let lines = line_count;
        let vec_mem_estimate = (lines as f64 * 150.0 + content.len() as f64) / (1024.0 * 1024.0);
        println!("  Estimated memory: ~{:.1} MB ({} lines * ~150 bytes + text)", vec_mem_estimate, lines);
    }

    println!("");
    println!("=== Summary ===");
    println!("For a {:.1} MB file with {} lines:", file_size_mb, line_count);
    if loaded.is_rope() {
        println!("  RopeBuffer was automatically selected (file > 1MB)");
        println!("  Load time: {:?} (vs potentially minutes with standard Buffer)", total_time);
    } else {
        println!("  Standard Buffer was used (file <= 1MB)");
    }
}
