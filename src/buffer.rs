use std::time::Instant;

use crate::{FontLayoutLine, FontLineIndex, FontMatches, FontShapeLine};

pub enum TextAction {
    Left,
    Right,
    Up,
    Down,
    Backspace,
    Delete,
    Insert(char),
}

#[derive(Default, Eq, PartialEq)]
pub struct TextCursor {
    pub line: usize,
    pub glyph: usize,
}

impl TextCursor {
    pub fn new(line: usize, glyph: usize) -> Self {
        Self { line, glyph }
    }
}

pub struct TextBuffer<'a> {
    font_matches: &'a FontMatches<'a>,
    text_lines: Vec<String>,
    shape_lines: Vec<FontShapeLine<'a>>,
    layout_lines: Vec<FontLayoutLine<'a>>,
    font_size: i32,
    line_width: i32,
    pub cursor: TextCursor,
    pub redraw: bool,
}

impl<'a> TextBuffer<'a> {
    pub fn new(font_matches: &'a FontMatches<'a>, text: &str, font_size: i32, line_width: i32) -> Self {
        let mut text_lines: Vec<String> = text.lines().map(String::from).collect();
        if text_lines.is_empty() {
            text_lines.push(String::new());
        }
        Self {
            font_matches,
            text_lines,
            shape_lines: Vec::new(),
            layout_lines: Vec::new(),
            font_size,
            line_width,
            cursor: TextCursor::default(),
            redraw: false,
        }
    }

    pub fn shape_until(&mut self, lines: i32) {
        let instant = Instant::now();

        let mut reshaped = 0;
        while self.shape_lines.len() < self.text_lines.len()
            && (self.layout_lines.len() as i32) < lines
        {
            let line_i = FontLineIndex::new(self.shape_lines.len());
            self.reshape_line(line_i);
            reshaped += 1;
        }

        let duration = instant.elapsed();
        if reshaped > 0 {
            log::debug!("shape_until {}: {:?}", reshaped, duration);
        }
    }

    pub fn reshape_line(&mut self, line_i: FontLineIndex) {
        let instant = Instant::now();

        let shape_line = self
            .font_matches
            .shape_line(line_i, &self.text_lines[line_i.get()]);
        if line_i.get() < self.shape_lines.len() {
            self.shape_lines[line_i.get()] = shape_line;
        } else {
            self.shape_lines.insert(line_i.get(), shape_line);
        }

        let duration = instant.elapsed();
        log::debug!("reshape line {}: {:?}", line_i.get(), duration);

        self.relayout_line(line_i);
    }

    pub fn relayout(&mut self) {
        let instant = Instant::now();

        self.layout_lines.clear();
        for line in self.shape_lines.iter() {
            let layout_i = self.layout_lines.len();
            line.layout(
                self.font_size,
                self.line_width,
                &mut self.layout_lines,
                layout_i,
            );
        }

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout: {:?}", duration);
    }

    pub fn relayout_line(&mut self, line_i: FontLineIndex) {
        let instant = Instant::now();

        let mut insert_opt = None;
        let mut layout_i = 0;
        while layout_i < self.layout_lines.len() {
            let layout_line = &self.layout_lines[layout_i];
            if layout_line.line_i == line_i {
                if insert_opt.is_none() {
                    insert_opt = Some(layout_i);
                }
                self.layout_lines.remove(layout_i);
            } else {
                layout_i += 1;
            }
        }

        let insert_i = insert_opt.unwrap_or(self.layout_lines.len());

        let shape_line = &self.shape_lines[line_i.get()];
        shape_line.layout(
            self.font_size,
            self.line_width,
            &mut self.layout_lines,
            insert_i,
        );

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout line {}: {:?}", line_i.get(), duration);
    }

    pub fn font_matches(&self) -> &FontMatches {
        &self.font_matches
    }

    pub fn font_size(&self) -> i32 {
        self.font_size
    }

    pub fn set_font_size(&mut self, font_size: i32) {
        self.font_size = font_size;
        self.relayout();
    }

    pub fn line_width(&self) -> i32 {
        self.line_width
    }

    pub fn set_line_width(&mut self, line_width: i32) {
        self.line_width = line_width;
        self.relayout();
    }

    pub fn layout_lines(&self) -> &[FontLayoutLine] {
        &self.layout_lines
    }

    pub fn text_lines(&self) -> &[String] {
        &self.text_lines
    }

    pub fn action(&mut self, action: TextAction) {
        match action {
            TextAction::Left => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph > line.glyphs.len() {
                    self.cursor.glyph = line.glyphs.len();
                    self.redraw = true;
                }
                if self.cursor.glyph > 0 {
                    self.cursor.glyph -= 1;
                    self.redraw = true;
                }
            }
            TextAction::Right => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph > line.glyphs.len() {
                    self.cursor.glyph = line.glyphs.len();
                    self.redraw = true;
                }
                if self.cursor.glyph < line.glyphs.len() {
                    self.cursor.glyph += 1;
                    self.redraw = true;
                }
            }
            TextAction::Up => {
                if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.redraw = true;
                }
            }
            TextAction::Down => {
                if self.cursor.line + 1 < self.layout_lines.len() {
                    self.cursor.line += 1;
                    self.redraw = true;
                }
            }
            TextAction::Backspace => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph > line.glyphs.len() {
                    self.cursor.glyph = line.glyphs.len();
                    self.redraw = true;
                }
                if self.cursor.glyph > 0 {
                    self.cursor.glyph -= 1;
                    let glyph = &line.glyphs[self.cursor.glyph];
                    let text_line = &mut self.text_lines[line.line_i.get()];
                    text_line.remove(glyph.start);
                    self.reshape_line(line.line_i);
                }
            }
            TextAction::Delete => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph < line.glyphs.len() {
                    let glyph = &line.glyphs[self.cursor.glyph];
                    let text_line = &mut self.text_lines[line.line_i.get()];
                    text_line.remove(glyph.start);
                    self.reshape_line(line.line_i);
                }
            }
            TextAction::Insert(character) => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph >= line.glyphs.len() {
                    match line.glyphs.last() {
                        Some(glyph) => {
                            let text_line = &mut self.text_lines[line.line_i.get()];
                            text_line.insert(glyph.end, character);
                            self.cursor.glyph += 1;
                            self.reshape_line(line.line_i);
                        }
                        None => {
                            let text_line = &mut self.text_lines[line.line_i.get()];
                            text_line.push(character);
                            self.cursor.glyph += 1;
                            self.reshape_line(line.line_i);
                        }
                    }
                } else {
                    let glyph = &line.glyphs[self.cursor.glyph];
                    let text_line = &mut self.text_lines[line.line_i.get()];
                    text_line.insert(glyph.start, character);
                    self.cursor.glyph += 1;
                    self.reshape_line(line.line_i);
                }
            }
        }
    }
}
