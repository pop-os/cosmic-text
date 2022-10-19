use std::{
    cmp,
    fmt,
    time::Instant,
};

use crate::{FontLayoutLine, FontMatches, FontShapeLine};

/// An action to perform on a [TextBuffer]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAction {
    /// Move cursor left
    Left,
    /// Move cursor right
    Right,
    /// Move cursor up
    Up,
    /// Move cursor down
    Down,
    /// Move cursor to start of line
    Home,
    /// Move cursor to end of line
    End,
    /// Scroll up one page
    PageUp,
    /// Scroll down one page
    PageDown,
    /// Insert character at cursor
    Insert(char),
    /// Create new line
    Enter,
    /// Delete text behind cursor
    Backspace,
    /// Delete text in front of cursor
    Delete,
    /// Mouse click at specified position
    Click { x: i32, y: i32 },
    /// Mouse drag to specified position
    Drag { x: i32, y: i32 },
    /// Scroll specified number of lines
    Scroll { lines: i32 },
}

/// Current cursor location
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextCursor {
    pub line: usize,
    pub glyph: usize,
}

impl TextCursor {
    pub const fn new(line: usize, glyph: usize) -> Self {
        Self { line, glyph }
    }
}

/// Index of a text line
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TextLineIndex(usize);

impl TextLineIndex {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

/// Metrics of text
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextMetrics {
    /// Font size in pixels
    pub font_size: i32,
    /// Line height in pixels
    pub line_height: i32,
}

impl TextMetrics {
    pub const fn new(font_size: i32, line_height: i32) -> Self {
        Self { font_size, line_height }
    }

    pub const fn scale(self, scale: i32) -> Self {
        Self {
            font_size: self.font_size * scale,
            line_height: self.line_height * scale,
        }
    }
}

impl fmt::Display for TextMetrics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}px / {}px", self.font_size, self.line_height)
    }
}

/// A buffer of text that is shaped and laid out
pub struct TextBuffer<'a> {
    font_matches: &'a FontMatches<'a>,
    text_lines: Vec<String>,
    shape_lines: Vec<FontShapeLine<'a>>,
    layout_lines: Vec<FontLayoutLine<'a>>,
    metrics: TextMetrics,
    width: i32,
    height: i32,
    scroll: i32,
    cursor: TextCursor,
    select_opt: Option<TextCursor>,
    pub redraw: bool,
}

impl<'a> TextBuffer<'a> {
    pub fn new(
        font_matches: &'a FontMatches<'a>,
        metrics: TextMetrics,
    ) -> Self {
        Self {
            font_matches,
            text_lines: Vec::new(),
            shape_lines: Vec::new(),
            layout_lines: Vec::new(),
            metrics,
            width: 0,
            height: 0,
            scroll: 0,
            cursor: TextCursor::default(),
            select_opt: None,
            redraw: false,
        }
    }

    /// Pre-shape lines in the buffer, up to `lines`
    pub fn shape_until(&mut self, lines: i32) {
        let instant = Instant::now();

        let mut reshaped = 0;
        while self.shape_lines.len() < self.text_lines.len()
            && (self.layout_lines.len() as i32) < lines
        {
            let line_i = TextLineIndex::new(self.shape_lines.len());
            self.reshape_line(line_i);
            reshaped += 1;
        }

        let duration = instant.elapsed();
        if reshaped > 0 {
            log::debug!("shape_until {}: {:?}", reshaped, duration);
        }
    }

    fn shape_until_scroll(&mut self) {
        let lines = self.lines();

        let scroll_end = self.scroll + lines;
        self.shape_until(scroll_end);

        self.scroll = cmp::max(
            0,
            cmp::min(
                self.layout_lines().len() as i32 - (lines - 1),
                self.scroll,
            ),
        );
    }

    fn reshape_line(&mut self, line_i: TextLineIndex) {
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

    fn relayout(&mut self) {
        let instant = Instant::now();

        self.layout_lines.clear();
        for line in self.shape_lines.iter() {
            let layout_i = self.layout_lines.len();
            line.layout(
                self.metrics.font_size,
                self.width,
                &mut self.layout_lines,
                layout_i,
            );
        }

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout: {:?}", duration);
    }

    fn relayout_line(&mut self, line_i: TextLineIndex) {
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
            self.metrics.font_size,
            self.width,
            &mut self.layout_lines,
            insert_i,
        );

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout line {}: {:?}", line_i.get(), duration);
    }

    /// Get the current cursor position
    pub fn cursor(&self) -> TextCursor {
        self.cursor
    }

    /// Get the current [TextMetrics]
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    /// Set the current [TextMetrics]
    pub fn set_metrics(&mut self, metrics: TextMetrics) {
        if metrics != self.metrics {
            self.metrics = metrics;
            self.relayout();
            self.shape_until_scroll();
        }
    }

    /// Get the current buffer dimensions (width, height)
    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    /// Set the current buffer dimensions
    pub fn set_size(&mut self, width: i32, height: i32) {
        if width != self.width {
            self.width = width;
            self.relayout();
            self.shape_until_scroll();
        }

        if height != self.height {
            self.height = height;
            self.shape_until_scroll();
        }
    }

    /// Get the current scroll location
    pub fn scroll(&self) -> i32 {
        self.scroll
    }

    /// Get the number of lines that can be viewed in the buffer
    pub fn lines(&self) -> i32 {
        self.height / self.metrics.line_height
    }

    /// Get the lines after layout for rendering
    pub fn layout_lines(&self) -> &[FontLayoutLine] {
        &self.layout_lines
    }

    /// Set text of buffer
    pub fn set_text(&mut self, text: &str) {
        self.text_lines = text.lines().map(String::from).collect();
        if self.text_lines.is_empty() {
            self.text_lines.push(String::new());
        }
        self.shape_lines.clear();
        self.layout_lines.clear();
        self.scroll = 0;
        self.cursor = TextCursor::default();
        self.select_opt = None;
        self.shape_until_scroll();
    }

    /// Get the lines of the original text
    pub fn text_lines(&self) -> &[String] {
        &self.text_lines
    }

    /// Perform a [TextAction] on the buffer
    pub fn action(&mut self, action: TextAction) {
        match action {
            TextAction::Left => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph > line.glyphs.len() {
                    self.cursor.glyph = line.glyphs.len();
                    self.redraw = true;
                }
                if line.rtl {
                    if self.cursor.glyph < line.glyphs.len() {
                        self.cursor.glyph += 1;
                        self.redraw = true;
                    }
                } else {
                    if self.cursor.glyph > 0 {
                        self.cursor.glyph -= 1;
                        self.redraw = true;
                    }
                }
            },
            TextAction::Right => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph > line.glyphs.len() {
                    self.cursor.glyph = line.glyphs.len();
                    self.redraw = true;
                }
                if line.rtl {
                    if self.cursor.glyph > 0 {
                        self.cursor.glyph -= 1;
                        self.redraw = true;
                    }
                } else {
                    if self.cursor.glyph < line.glyphs.len() {
                        self.cursor.glyph += 1;
                        self.redraw = true;
                    }
                }
            },
            TextAction::Up => {
                if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.redraw = true;

                    let lines = self.lines();
                    if (self.cursor.line as i32) < self.scroll
                    || (self.cursor.line as i32) >= self.scroll + lines
                    {
                        self.scroll = self.cursor.line as i32;
                    }
                }
            },
            TextAction::Down => {
                if self.cursor.line < self.layout_lines.len() {
                    self.cursor.line += 1;
                    self.redraw = true;

                    let lines = self.lines();
                    if (self.cursor.line as i32) < self.scroll
                    || (self.cursor.line as i32) >= self.scroll + lines
                    {
                        self.scroll = self.cursor.line as i32 - (lines - 1);
                        self.shape_until_scroll();
                    }
                }
            },
            TextAction::Home => {
                if self.cursor.glyph > 0 {
                    self.cursor.glyph = 0;
                    self.redraw = true;
                }
            },
            TextAction::End => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph < line.glyphs.len() {
                    self.cursor.glyph = line.glyphs.len();
                    self.redraw = true;
                }
            }
            TextAction::PageUp => {
                self.scroll -= self.lines();
                self.redraw = true;
                self.shape_until_scroll();
            },
            TextAction::PageDown => {
                self.scroll += self.lines();
                self.redraw = true;
                self.shape_until_scroll();
            },
            TextAction::Insert(character) => if character.is_control() {
                // Filter out special chars, use TextAction instead
                log::debug!("Refusing to insert control character {:?}", character);
            } else {
                let line = &self.layout_lines[self.cursor.line];
                let insert_i = if self.cursor.glyph >= line.glyphs.len() {
                    match line.glyphs.last() {
                        Some(glyph) => glyph.end,
                        None => self.text_lines[line.line_i.get()].len()
                    }
                } else {
                    line.glyphs[self.cursor.glyph].start
                };

                let text_line = &mut self.text_lines[line.line_i.get()];
                text_line.insert(insert_i, character);
                self.cursor.glyph += 1;
                self.reshape_line(line.line_i);
            },
            TextAction::Enter => {
                {
                    let line = &self.layout_lines[self.cursor.line];
                    let new_line = if self.cursor.glyph >= line.glyphs.len() {
                        String::new()
                    } else {
                        let glyph = &line.glyphs[self.cursor.glyph];
                        self.text_lines[line.line_i.get()].split_off(glyph.start)
                    };
                    self.text_lines.insert(line.line_i.get() + 1, new_line);

                    // Reshape all lines after new line
                    //TODO: improve performance
                    self.shape_lines.truncate(line.line_i.get());
                    self.relayout();
                    self.shape_until_scroll();
                }

                self.cursor.glyph = 0;
                self.cursor.line += 1;

                let lines = self.lines();
                if (self.cursor.line as i32) < self.scroll
                || (self.cursor.line as i32) >= self.scroll + lines
                {
                    self.scroll = self.cursor.line as i32 - (lines - 1);
                    self.shape_until_scroll();
                }
            },
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
                } else if self.cursor.line > 0 {
                    self.cursor.glyph = self.layout_lines[self.cursor.line - 1].glyphs.len();

                    {
                        let line = &self.layout_lines[self.cursor.line];
                        let old_line = self.text_lines.remove(line.line_i.get());
                        self.text_lines[line.line_i.get() - 1].push_str(&old_line);

                        // Reshape all lines after new line
                        //TODO: improve performance
                        self.shape_lines.truncate(line.line_i.get() - 1);
                        self.relayout();
                        self.shape_until_scroll();
                    }

                    self.cursor.line -= 1;

                    let lines = self.lines();
                    if (self.cursor.line as i32) < self.scroll
                    || (self.cursor.line as i32) >= self.scroll + lines
                    {
                        self.scroll = self.cursor.line as i32;
                    }
                }
            },
            TextAction::Delete => {
                let line = &self.layout_lines[self.cursor.line];
                if self.cursor.glyph < line.glyphs.len() {
                    let glyph = &line.glyphs[self.cursor.glyph];
                    let text_line = &mut self.text_lines[line.line_i.get()];
                    text_line.remove(glyph.start);
                    self.reshape_line(line.line_i);
                } else {
                    self.shape_until(self.cursor.line as i32 + 1);

                    if self.cursor.line + 1 < self.layout_lines.len() {
                        let line = &self.layout_lines[self.cursor.line];
                        let next_line = &self.layout_lines[self.cursor.line + 1];
                        if line.line_i.get() < next_line.line_i.get() {
                            let old_line = self.text_lines.remove(next_line.line_i.get());
                            self.text_lines[line.line_i.get()].push_str(&old_line);
                        } else {
                            match line.glyphs.last() {
                                Some(glyph) => {
                                    let text_line = &mut self.text_lines[line.line_i.get()];
                                    text_line.remove(glyph.end);
                                },
                                None => (), // There should always be a last glyph
                            }
                        }

                        // Reshape all lines after new line
                        //TODO: improve performance
                        self.shape_lines.truncate(line.line_i.get());
                        self.relayout();
                        self.shape_until_scroll();
                    }
                }
            },
            TextAction::Click { x, y } => {
                self.select_opt = None;
                self.click(x, y);
            },
            TextAction::Drag { x, y } => {
                if self.select_opt.is_none() {
                    self.select_opt = Some(self.cursor);
                    self.redraw = true;
                }
                self.click(x, y);
            },
            TextAction::Scroll { lines } => {
                self.scroll += lines;
                self.redraw = true;
                self.shape_until_scroll();
            }
        }
    }

    fn click(&mut self, mouse_x: i32, mouse_y: i32) {
        let instant = Instant::now();

        let font_size = self.metrics.font_size;
        let line_height = self.metrics.line_height;

        let mut new_cursor_opt = None;

        let mut line_y = font_size;
        for (line_i, line) in self.layout_lines.iter()
            .skip(cmp::max(0, self.scroll()) as usize)
            .take(cmp::max(0, self.lines()) as usize)
            .enumerate()
        {
            if mouse_y >= line_y - font_size
            && mouse_y < line_y - font_size + line_height
            {
                let new_cursor_line = line_i + self.scroll() as usize;
                let mut new_cursor_glyph = line.glyphs.len();
                for (glyph_i, glyph) in line.glyphs.iter().enumerate() {
                    if mouse_x >= glyph.x as i32
                        && mouse_x <= (glyph.x + glyph.w) as i32
                    {
                        let right_half = mouse_x >= (glyph.x + glyph.w / 2.0) as i32;
                        if right_half == !line.rtl {
                            // If clicking on last half of glyph, move cursor past glyph
                            new_cursor_glyph = glyph_i + 1;
                        } else {
                            new_cursor_glyph = glyph_i;
                        }
                    }
                }
                new_cursor_opt = Some(TextCursor::new(new_cursor_line, new_cursor_glyph));

                if let Some(glyph) = line.glyphs.get(new_cursor_glyph) {
                    let text_line = &self.text_lines[line.line_i.get()];
                    let text_glyph = &text_line[glyph.start..glyph.end];
                    log::debug!(
                        "{}, {}: '{}' ('{}'): '{}' ({:?})",
                        new_cursor_line,
                        new_cursor_glyph,
                        glyph.font.info.family,
                        glyph.font.info.post_script_name,
                        text_glyph,
                        text_glyph
                    );
                }
            }

            line_y += line_height;
        }

        if let Some(new_cursor) = new_cursor_opt {
            if new_cursor != self.cursor {
                self.cursor = new_cursor;
                self.redraw = true;
            }
        }

        let duration = instant.elapsed();
        log::trace!("click({}, {}): {:?}", mouse_x, mouse_y, duration);
    }

    /// Draw the buffer
    pub fn draw<F>(&self, color: u32, mut f: F)
        where F: FnMut(i32, i32, u32, u32, u32)
    {
        let font_size = self.metrics.font_size;
        let line_height = self.metrics.line_height;

        let mut line_y = font_size;
        for (line_i, line) in self.layout_lines.iter()
            .skip(cmp::max(0, self.scroll()) as usize)
            .take(cmp::max(0, self.lines()) as usize)
            .enumerate()
        {
            let line_i_scrolled = line_i + cmp::max(0, self.scroll()) as usize;

            // Highlight selection (TODO: HIGHLIGHT COLOR!)
            if let Some(select) = self.select_opt {
                let (start, end) = if select.line < self.cursor.line {
                    (select, self.cursor)
                } else if select.line > self.cursor.line {
                    (self.cursor, select)
                } else {
                    /* select.line == self.cursor.line */
                    if select.glyph < self.cursor.glyph {
                        (select, self.cursor)
                    } else {
                        /* select.glyph >= self.cursor.glyph */
                        (self.cursor, select)
                    }
                };

                if line_i_scrolled >= start.line && line_i_scrolled <= end.line {
                    let start_glyph = if start.line == line_i_scrolled {
                        start.glyph
                    } else {
                        0
                    };

                    let end_glyph = if end.line == line_i_scrolled {
                        end.glyph
                    } else {
                        line.glyphs.len() + 1
                    };

                    if end_glyph > start_glyph {
                        let (left_x, right_x) = if line.rtl {
                            (
                                line.glyphs.get(end_glyph - 1).map_or(0, |glyph| {
                                    glyph.x as i32
                                }),
                                line.glyphs.get(start_glyph).map_or(self.width, |glyph| {
                                    (glyph.x + glyph.w) as i32
                                }),
                            )
                        } else {
                            (
                                line.glyphs.get(start_glyph).map_or(0, |glyph| {
                                    glyph.x as i32
                                }),
                                line.glyphs.get(end_glyph - 1).map_or(self.width, |glyph| {
                                    (glyph.x + glyph.w) as i32
                                }),
                            )
                        };

                        f(
                            left_x,
                            line_y - font_size,
                            cmp::max(0, right_x - left_x) as u32,
                            line_height as u32,
                            0x33_00_00_00 | (color & 0xFF_FF_FF)
                        );
                    }
                }
            }

            // Draw cursor
            if self.cursor.line == line_i_scrolled {
                if self.cursor.glyph >= line.glyphs.len() {
                    let x = match line.glyphs.last() {
                        Some(glyph) => glyph.x + glyph.w,
                        None => 0.0,
                    };

                    f(
                        x as i32,
                        line_y - font_size,
                        1,
                        line_height as u32,
                        color,
                    );
                } else {
                    let glyph = &line.glyphs[self.cursor.glyph];
                    let x = if line.rtl {
                        (glyph.x + glyph.w) as i32
                    } else {
                        glyph.x as i32
                    };

                    f(
                        x,
                        line_y - font_size,
                        1,
                        line_height as u32,
                        color,
                    );
                }
            }

            line.draw(color, |x, y, color| {
                f(x, line_y + y, 1, 1, color);
            });

            line_y += line_height;
        }
    }
}
