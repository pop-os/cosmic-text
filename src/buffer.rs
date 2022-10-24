// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    cmp,
    fmt,
    time::Instant,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{FontLayoutLine, FontMatches, FontShapeLine};

/// An action to perform on a [TextBuffer]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAction {
    /// Move cursor to previous character ([Left] in LTR, [Right] in RTL)
    Previous,
    /// Move cursor to next character ([Right] in LTR, [Left] in RTL)
    Next,
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
    /// Text line the cursor is on
    pub line: TextLineIndex,
    /// Index of glyph at cursor (will insert behind this glyph)
    pub index: usize,
}

impl TextCursor {
    pub const fn new(line: TextLineIndex, index: usize) -> Self {
        Self { line, index }
    }
}

struct LayoutCursor {
    line: TextLineIndex,
    layout: usize,
    glyph: usize,
}

impl LayoutCursor {
    fn new(line: TextLineIndex, layout: usize, glyph: usize) -> Self {
        Self { line, layout, glyph }
    }
}

/// Index of a text line
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
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

pub struct TextBufferLine<'a> {
    text: String,
    shape_opt: Option<FontShapeLine<'a>>,
    layout_opt: Option<Vec<FontLayoutLine<'a>>>,
}

impl<'a> TextBufferLine<'a> {
    pub fn new(text: String) -> Self {
        Self {
            text,
            shape_opt: None,
            layout_opt: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn reset(&mut self) {
        self.shape_opt = None;
        self.layout_opt = None;
    }

    pub fn shape(&mut self, font_matches: &'a FontMatches<'a>) -> &FontShapeLine<'a> {
        if self.shape_opt.is_none() {
            self.shape_opt = Some(font_matches.shape_line(&self.text));
            self.layout_opt = None;
        }
        self.shape_opt.as_ref().unwrap()
    }

    pub fn layout(&mut self, font_matches: &'a FontMatches<'a>, font_size: i32, width: i32) -> &[FontLayoutLine<'a>] {
        if self.layout_opt.is_none() {
            let mut layout = Vec::new();
            let shape = self.shape(font_matches);
            shape.layout(
                font_size,
                width,
                &mut layout,
                0,
            );
            self.layout_opt = Some(layout);
        }
        self.layout_opt.as_ref().unwrap()
    }
}

/// A buffer of text that is shaped and laid out
pub struct TextBuffer<'a> {
    font_matches: &'a FontMatches<'a>,
    lines: Vec<TextBufferLine<'a>>,
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
        let mut buffer = Self {
            font_matches,
            lines: Vec::new(),
            metrics,
            width: 0,
            height: 0,
            scroll: 0,
            cursor: TextCursor::default(),
            select_opt: None,
            redraw: false,
        };
        buffer.set_text("");
        buffer
    }

    /// Pre-shape lines in the buffer, up to `lines`, return actual number of layout lines
    pub fn shape_until(&mut self, lines: i32) -> i32 {
        let instant = Instant::now();

        let mut reshaped = 0;
        let mut total_layout = 0;
        for line in self.lines.iter_mut() {
            if total_layout >= lines {
                break;
            }

            if line.shape_opt.is_none() {
                reshaped += 1;
            }
            let layout = line.layout(
                self.font_matches,
                self.metrics.font_size,
                self.width
            );
            total_layout += layout.len() as i32;
        }

        let duration = instant.elapsed();
        if reshaped > 0 {
            log::debug!("shape_until {}: {:?}", reshaped, duration);
            self.redraw = true;
        }

        total_layout
    }

    pub fn shape_until_cursor(&mut self) {
        let instant = Instant::now();

        let mut reshaped = 0;
        let mut layout_i = 0;
        for (line_i, line) in self.lines.iter_mut().enumerate() {
            if line_i > self.cursor.line.get() {
                break;
            }

            if line.shape_opt.is_none() {
                reshaped += 1;
            }
            let layout = line.layout(
                self.font_matches,
                self.metrics.font_size,
                self.width
            );
            if line_i == self.cursor.line.get() {
                for layout_line in layout {
                    let mut found = false;
                    for glyph in layout_line.glyphs.iter() {
                        if glyph.start <= self.cursor.index {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        layout_i += 1;
                    } else {
                        break;
                    }
                }
            } else {
                layout_i += layout.len() as i32;
            }
        }

        let duration = instant.elapsed();
        if reshaped > 0 {
            log::debug!("shape_until_cursor {}: {:?}", reshaped, duration);
            self.redraw = true;
        }

        let lines = self.lines();
        if layout_i < self.scroll {
            self.scroll = layout_i;
        } else if layout_i >= self.scroll + lines {
            self.scroll = layout_i - (lines - 1);
        }

        self.shape_until_scroll();
    }

    fn shape_until_scroll(&mut self) {
        let lines = self.lines();

        let scroll_end = self.scroll + lines;
        let total_layout = self.shape_until(scroll_end);

        self.scroll = cmp::max(
            0,
            cmp::min(
                total_layout - (lines - 1),
                self.scroll,
            ),
        );
    }

    fn relayout(&mut self) {
        let instant = Instant::now();

        for line in self.lines.iter_mut() {
            if line.shape_opt.is_some() {
                line.layout_opt = None;
                line.layout(
                    self.font_matches,
                    self.metrics.font_size,
                    self.width
                );
            }
        }

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout: {:?}", duration);
    }

    fn layout_cursor(&self, cursor: &TextCursor) -> LayoutCursor {
        let line = &self.lines[cursor.line.get()];

        let layout = line.layout_opt.as_ref().unwrap(); //TODO: ensure layout is done?
        for (layout_i, layout_line) in layout.iter().enumerate() {
            for (glyph_i, glyph) in layout_line.glyphs.iter().enumerate() {
                if cursor.index == glyph.start {
                    return LayoutCursor::new(
                        cursor.line,
                        layout_i,
                        glyph_i
                    );
                }
            }
            match layout_line.glyphs.last() {
                Some(glyph) => {
                    if cursor.index == glyph.end {
                        return LayoutCursor::new(
                            cursor.line,
                            layout_i,
                            layout_line.glyphs.len()
                        );
                    }
                },
                None => {
                    return LayoutCursor::new(
                        cursor.line,
                        layout_i,
                        0
                    );
                }
            }
        }

        // Fall back to start of line
        //TODO: should this be the end of the line?
        LayoutCursor::new(
            cursor.line,
            0,
            0
        )
    }

    fn set_layout_cursor(&mut self, cursor: LayoutCursor) {
        let line = &mut self.lines[cursor.line.get()];
        let layout = line.layout(
            &mut self.font_matches,
            self.metrics.font_size,
            self.width
        );

        let layout_line = match layout.get(cursor.layout) {
            Some(some) => some,
            None => match layout.last() {
                Some(some) => some,
                None => todo!("layout cursor in line with no layouts"),
            }
        };

        let new_index = match layout_line.glyphs.get(cursor.glyph) {
            Some(glyph) => glyph.start,
            None => match layout_line.glyphs.last() {
                Some(glyph) => glyph.end,
                //TODO: is this correct?
                None => 0,
            }
        };

        if self.cursor.line != cursor.line || self.cursor.index != new_index {
            self.cursor.line = cursor.line;
            self.cursor.index = new_index;
            self.redraw = true;
        }
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

    /// Set text of buffer
    pub fn set_text(&mut self, text: &str) {
        self.lines.clear();
        for line in text.lines() {
            self.lines.push(TextBufferLine::new(line.to_string()));
        }
        // Make sure there is always one line
        if self.lines.is_empty() {
            self.lines.push(TextBufferLine::new(String::new()));
        }

        self.scroll = 0;
        self.cursor = TextCursor::default();
        self.select_opt = None;

        self.shape_until_scroll();
    }

    /// Get the lines of the original text
    pub fn text_lines(&self) -> &[TextBufferLine<'a>] {
        &self.lines
    }

    /// Perform a [TextAction] on the buffer
    pub fn action(&mut self, action: TextAction) {
        match action {
            TextAction::Previous => {
                let line = &mut self.lines[self.cursor.line.get()];

                if self.cursor.index > 0 {
                    // Find previous character index
                    let mut prev_index = 0;
                    for (i, _) in line.text.grapheme_indices(true) {
                        if i < self.cursor.index {
                            prev_index = i;
                        } else {
                            break;
                        }
                    }

                    self.cursor.index = prev_index;
                    self.redraw = true;
                } else if self.cursor.line.get() > 0 {
                    self.cursor.line = TextLineIndex::new(self.cursor.line.get() - 1);
                    self.cursor.index = self.lines[self.cursor.line.get()].text.len();
                    self.redraw = true;
                }
            },
            TextAction::Next => {
                let line = &mut self.lines[self.cursor.line.get()];

                if self.cursor.index < line.text.len() {
                    for (i, c) in line.text.grapheme_indices(true) {
                        if i == self.cursor.index {
                            self.cursor.index += c.len();
                            self.redraw = true;
                            break;
                        }
                    }
                } else if self.cursor.line.get() + 1 < self.lines.len() {
                    self.cursor.line = TextLineIndex::new(self.cursor.line.get() + 1);
                    self.cursor.index = 0;
                    self.redraw = true;
                }
            },
            TextAction::Left => {
                let rtl_opt = self.lines[self.cursor.line.get()].shape_opt.as_ref().map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(TextAction::Next);
                    } else {
                        self.action(TextAction::Previous);
                    }
                }
            },
            TextAction::Right => {
                let rtl_opt = self.lines[self.cursor.line.get()].shape_opt.as_ref().map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(TextAction::Previous);
                    } else {
                        self.action(TextAction::Next);
                    }
                }
            },
            TextAction::Up => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.layout_cursor(&self.cursor);
                if cursor.layout > 0 {
                    cursor.layout -= 1;
                } else if cursor.line.get() > 0 {
                    cursor.line = TextLineIndex::new(cursor.line.get() - 1);
                    cursor.layout = usize::max_value();
                }
                self.set_layout_cursor(cursor);
            },
            TextAction::Down => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.layout_cursor(&self.cursor);
                let layout_len = {
                    let line = &mut self.lines[cursor.line.get()];
                    let layout = line.layout(
                        &mut self.font_matches,
                        self.metrics.font_size,
                        self.width
                    );
                    layout.len()
                };
                if cursor.layout + 1 < layout_len {
                    cursor.layout += 1;
                } else if cursor.line.get() + 1 < self.lines.len() {
                    cursor.line = TextLineIndex::new(cursor.line.get() + 1);
                    cursor.layout = 0;
                }
                self.set_layout_cursor(cursor);
            },
            TextAction::Home => {
                let mut cursor = self.layout_cursor(&self.cursor);
                cursor.glyph = 0;
                self.set_layout_cursor(cursor);
            },
            TextAction::End => {
                let mut cursor = self.layout_cursor(&self.cursor);
                cursor.glyph = usize::max_value();
                self.set_layout_cursor(cursor);
            }
            TextAction::PageUp => {
                //TODO: move cursor
                self.scroll -= self.lines();
                self.redraw = true;

                self.shape_until_scroll();
            },
            TextAction::PageDown => {
                //TODO: move cursor
                self.scroll += self.lines();
                self.redraw = true;

                self.shape_until_scroll();
            },
            TextAction::Insert(character) => {
                if character.is_control()
                && !['\t', '\u{92}'].contains(&character)
                {
                    // Filter out special chars (except for tab), use TextAction instead
                    log::debug!("Refusing to insert control character {:?}", character);
                } else {
                    let line = &mut self.lines[self.cursor.line.get()];
                    line.reset();
                    line.text.insert(self.cursor.index, character);

                    self.cursor.index += character.len_utf8();
                }
            },
            TextAction::Enter => {
                let new_line = {
                    let line = &mut self.lines[self.cursor.line.get()];
                    line.reset();
                    line.text.split_off(self.cursor.index)
                };

                let next_line = self.cursor.line.get() + 1;
                self.lines.insert(next_line, TextBufferLine::new(new_line));

                self.cursor.line = TextLineIndex::new(next_line);
                self.cursor.index = 0;
            },
            TextAction::Backspace => {
                if self.cursor.index > 0 {
                    let line = &mut self.lines[self.cursor.line.get()];
                    line.reset();

                    // Find previous character index
                    let mut prev_index = 0;
                    for (i, _) in line.text.char_indices() {
                        if i < self.cursor.index {
                            prev_index = i;
                        } else {
                            break;
                        }
                    }

                    self.cursor.index = prev_index;

                    line.text.remove(self.cursor.index);
                } else if self.cursor.line.get() > 0 {
                    let mut line_index = self.cursor.line.get();
                    let old_line = self.lines.remove(line_index);
                    line_index -= 1;

                    let line = &mut self.lines[line_index];
                    line.reset();

                    self.cursor.line = TextLineIndex::new(line_index);
                    self.cursor.index = line.text.len();

                    line.text.push_str(&old_line.text);
                }
            },
            TextAction::Delete => {
                if self.cursor.index < self.lines[self.cursor.line.get()].text.len() {
                    let line = &mut self.lines[self.cursor.line.get()];
                    line.reset();

                    if let Some((i, c)) = line
                        .text
                        .grapheme_indices(true)
                        .take_while(|(i, c)| *i <= self.cursor.index)
                        .last()
                    {
                        line.text.replace_range(i..(i + c.len()), "");
                        self.cursor.index = i;
                    }
                } else if self.cursor.line.get() + 1 < self.lines.len() {
                    let old_line = self.lines.remove(self.cursor.line.get() + 1);

                    let line = &mut self.lines[self.cursor.line.get()];
                    line.reset();

                    line.text.push_str(&old_line.text);
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

        let mut line_y = font_size;
        let mut layout_i = 0;
        for (line_i, line) in self.lines.iter().enumerate() {
            let shape = match line.shape_opt.as_ref() {
                Some(some) => some,
                None => break,
            };

            let layout = match line.layout_opt.as_ref() {
                Some(some) => some,
                None => break,
            };

            for layout_line in layout {
                let scrolled = layout_i < self.scroll;
                layout_i += 1;

                if scrolled {
                    continue;
                }

                if line_y > self.height {
                    return;
                }

                if mouse_y >= line_y - font_size
                && mouse_y < line_y - font_size + line_height
                {
                    let mut new_cursor_glyph = layout_line.glyphs.len();
                    let mut new_cursor_char = 0;
                    'hit: for (glyph_i, glyph) in layout_line.glyphs.iter().enumerate() {
                        if mouse_x >= glyph.x as i32
                        && mouse_x <= (glyph.x + glyph.w) as i32
                        {
                            new_cursor_glyph = glyph_i;

                            let cluster = &line.text[glyph.start..glyph.end];
                            let total = cluster.grapheme_indices(true).count();
                            let mut egc_x = glyph.x;
                            let egc_w = glyph.w / (total as f32);
                            for (egc_i, egc) in cluster.grapheme_indices(true) {
                                if mouse_x >= egc_x as i32
                                && mouse_x <= (egc_x + egc_w) as i32
                                {
                                    new_cursor_char = egc_i;

                                    let right_half = mouse_x >= (egc_x + egc_w / 2.0) as i32;
                                    if right_half == !shape.rtl {
                                        // If clicking on last half of glyph, move cursor past glyph
                                        new_cursor_char += egc.len();
                                        if new_cursor_char >= cluster.len() {
                                            new_cursor_glyph += 1;
                                            new_cursor_char = 0;
                                        }
                                    }
                                    break 'hit;
                                }
                                egc_x += egc_w;
                            }

                            let right_half = mouse_x >= (glyph.x + glyph.w / 2.0) as i32;
                            if right_half == !shape.rtl {
                                // If clicking on last half of glyph, move cursor past glyph
                                new_cursor_glyph += 1;
                            }
                            break 'hit;
                        }
                    }

                    let mut new_cursor = TextCursor::new(TextLineIndex::new(line_i), 0);

                    match layout_line.glyphs.get(new_cursor_glyph) {
                        Some(glyph) => {
                            // Position at glyph
                            new_cursor.index = glyph.start + new_cursor_char;
                        },
                        None => match layout_line.glyphs.last() {
                            Some(glyph) => {
                                // Position at end of line
                                new_cursor.index = glyph.end;
                            },
                            None => {
                                // Keep at start of empty line
                            },
                        },
                    }

                    if new_cursor != self.cursor {
                        self.cursor = new_cursor;
                        self.redraw = true;

                        if let Some(glyph) = layout_line.glyphs.get(new_cursor_glyph) {
                            let text_glyph = &line.text[glyph.start..glyph.end];
                            log::debug!(
                                "{}, {}: '{}' ('{}'): '{}' ({:?})",
                                self.cursor.line.get(),
                                self.cursor.index,
                                glyph.font.info.family,
                                glyph.font.info.post_script_name,
                                text_glyph,
                                text_glyph
                            );
                        }
                    }
                }

                line_y += line_height;
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
        /*TODO
        let layout_cursor = self.layout_cursor(&self.cursor);
        let layout_select_opt = self.select_opt.as_ref().map(|cursor| {
            self.layout_cursor(cursor)
        });
        */

        let mut line_y = font_size;
        let mut layout_i = 0;
        for (line_i, line) in self.lines.iter().enumerate() {
            let shape = match line.shape_opt.as_ref() {
                Some(some) => some,
                None => break,
            };

            let layout = match line.layout_opt.as_ref() {
                Some(some) => some,
                None => break,
            };

            for layout_line in layout {
                let scrolled = layout_i < self.scroll;
                layout_i += 1;

                if scrolled {
                    continue;
                }

                if line_y > self.height {
                    return;
                }

                let cursor_glyph_opt = |cursor: &TextCursor| -> Option<(usize, f32)> {
                    if cursor.line.get() == line_i {
                        for (glyph_i, glyph) in layout_line.glyphs.iter().enumerate() {
                            if cursor.index == glyph.start {
                                return Some((glyph_i, 0.0));
                            } else if cursor.index > glyph.start && cursor.index < glyph.end {
                                // Guess x offset based on characters
                                let mut before = 0;
                                let mut total = 0;

                                let cluster = &line.text[glyph.start..glyph.end];
                                for (i, _) in cluster.grapheme_indices(true) {
                                    if glyph.start + i < cursor.index {
                                        before += 1;
                                    }
                                    total += 1;
                                }

                                let offset = glyph.w * (before as f32) / (total as f32);
                                return Some((glyph_i, offset));
                            }
                        }
                        match layout_line.glyphs.last() {
                            Some(glyph) => {
                                if cursor.index == glyph.end {
                                    return Some((layout_line.glyphs.len(), 0.0));
                                }
                            },
                            None => {
                                return Some((0, 0.0));
                            }
                        }
                    }
                    None
                };

                // Highlight selection (TODO: HIGHLIGHT COLOR!)
                if let Some(select) = self.select_opt {
                    let (start, end) = if select.line < self.cursor.line {
                        (select, self.cursor)
                    } else if select.line > self.cursor.line {
                        (self.cursor, select)
                    } else {
                        /* select.line == self.cursor.line */
                        if select.index < self.cursor.index {
                            (select, self.cursor)
                        } else {
                            /* select.index >= self.cursor.index */
                            (self.cursor, select)
                        }
                    };

                    //TODO: do not calculate these on every line draw
                    let start_layout = self.layout_cursor(&start);
                    let end_layout = self.layout_cursor(&end);

                    // Check if this layout line is inside the selection
                    let mut inside = false;
                    if line_i > start.line.get() && line_i < end.line.get() {
                        // In between start and end lines, definitely inside selection
                        inside = true;
                    } else {
                        if line_i == start.line.get() && line_i == end.line.get() {
                            // On edge of start and end line, check if any contained glyphs are after start and before end
                            for glyph in layout_line.glyphs.iter() {
                                if glyph.end > start.index && glyph.start < end.index {
                                    inside = true;
                                    break;
                                }
                            }
                        } else if line_i == start.line.get() {
                            // On edge of start line, check if any contained glyphs are after start
                            for glyph in layout_line.glyphs.iter() {
                                if glyph.end > start.index {
                                    inside = true;
                                    break;
                                }
                            }
                        } else if line_i == end.line.get() {
                            // On edge of end line, check if any contained glyphs are before end
                            for glyph in layout_line.glyphs.iter() {
                                if glyph.start < end.index {
                                    inside = true;
                                    break;
                                }
                            }
                        }
                    }

                    if inside {
                        let (start_glyph, start_glyph_offset) = if start.line.get() == line_i {
                            cursor_glyph_opt(&start).unwrap_or((0, 0.0))
                        } else {
                            (0, 0.0)
                        };

                        let (end_glyph, end_glyph_offset) = if end.line.get() == line_i {
                            cursor_glyph_opt(&end).unwrap_or((layout_line.glyphs.len() + 1, 0.0))
                        } else {
                            (layout_line.glyphs.len() + 1, 0.0)
                        };

                        if end_glyph > start_glyph
                        || (end_glyph == start_glyph && end_glyph_offset > start_glyph_offset)
                        {
                            let (left_x, right_x) = if shape.rtl {
                                (
                                    layout_line.glyphs.get(end_glyph - 1).map_or(0, |glyph| {
                                        (glyph.x - end_glyph_offset) as i32
                                    }),
                                    layout_line.glyphs.get(start_glyph).map_or(self.width, |glyph| {
                                        (glyph.x + glyph.w - start_glyph_offset) as i32
                                    }),
                                )
                            } else {
                                (
                                    layout_line.glyphs.get(start_glyph).map_or(0, |glyph| {
                                        (glyph.x + start_glyph_offset) as i32
                                    }),
                                    layout_line.glyphs.get(end_glyph - 1).map_or(self.width, |glyph| {
                                        (glyph.x + glyph.w + end_glyph_offset) as i32
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
                //TODO: draw at end of line but not start of next line
                if let Some((cursor_glyph, cursor_glyph_offset)) = cursor_glyph_opt(&self.cursor) {
                    let x = match layout_line.glyphs.get(cursor_glyph) {
                        Some(glyph) => {
                            // Start of detected glyph
                            if shape.rtl {
                                (glyph.x + glyph.w - cursor_glyph_offset) as i32
                            } else {
                                (glyph.x + cursor_glyph_offset) as i32
                            }
                        },
                        None => match layout_line.glyphs.last() {
                            Some(glyph) => {
                                // End of last glyph
                                if shape.rtl {
                                    glyph.x as i32
                                } else {
                                    (glyph.x + glyph.w) as i32
                                }
                            },
                            None => {
                                // Start of empty line
                                0
                            }
                        }
                    };

                    f(
                        x,
                        line_y - font_size,
                        1,
                        line_height as u32,
                        color,
                    );
                }

                layout_line.draw(color, |x, y, color| {
                    f(x, line_y + y, 1, 1, color);
                });

                line_y += line_height;
            }
        }
    }
}
