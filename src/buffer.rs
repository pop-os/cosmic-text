// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    cmp,
    fmt,
    time::Instant,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Attrs, AttrsSpan, FontSystem, LayoutGlyph, LayoutLine, ShapeLine};

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
    pub line: usize,
    /// Index of glyph at cursor (will insert behind this glyph)
    pub index: usize,
}

impl TextCursor {
    pub const fn new(line: usize, index: usize) -> Self {
        Self { line, index }
    }
}

struct TextLayoutCursor {
    line: usize,
    layout: usize,
    glyph: usize,
}

impl TextLayoutCursor {
    fn new(line: usize, layout: usize, glyph: usize) -> Self {
        Self { line, layout, glyph }
    }
}

pub struct TextLayoutRun<'a> {
    /// The index of the original text line
    pub line_i: usize,
    /// The original text line
    pub text: &'a str,
    /// True if the original paragraph direction is RTL
    pub rtl: bool,
    /// The array of layout glyphs to draw
    pub glyphs: &'a [LayoutGlyph],
    /// Y offset of line
    pub line_y: i32,
}

pub struct TextLayoutRunIter<'a, 'b> {
    buffer: &'b TextBuffer<'a>,
    line_i: usize,
    layout_i: usize,
    line_y: i32,
    total_layout: i32,
}

impl<'a, 'b> TextLayoutRunIter<'a, 'b> {
    pub fn new(buffer: &'b TextBuffer<'a>) -> Self {
        Self {
            buffer,
            line_i: 0,
            layout_i: 0,
            line_y: buffer.metrics.font_size - buffer.metrics.line_height,
            total_layout: 0,
        }
    }
}

impl<'a, 'b> Iterator for TextLayoutRunIter<'a, 'b> {
    type Item = TextLayoutRun<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.buffer.lines.get(self.line_i) {
            let shape = line.shape_opt.as_ref()?;
            let layout = line.layout_opt.as_ref()?;
            while let Some(layout_line) = layout.get(self.layout_i) {
                self.layout_i += 1;

                let scrolled = self.total_layout < self.buffer.scroll;
                self.total_layout += 1;
                if scrolled {
                    continue;
                }

                self.line_y += self.buffer.metrics.line_height;
                if self.line_y > self.buffer.height {
                    return None;
                }

                return Some(TextLayoutRun {
                    line_i: self.line_i,
                    text: line.text.as_str(),
                    rtl: shape.rtl,
                    glyphs: &layout_line.glyphs,
                    line_y: self.line_y,
                });
            }
            self.line_i += 1;
            self.layout_i = 0;
        }

        None
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
    pub text: String,
    pub attrs_spans: Vec<AttrsSpan<'a>>,
    shape_opt: Option<ShapeLine>,
    layout_opt: Option<Vec<LayoutLine>>,
}

impl<'a> TextBufferLine<'a> {
    pub fn new(text: String, attrs: Attrs<'a>) -> Self {
        let attrs_spans = vec![AttrsSpan {
            start: 0,
            end: text.len(),
            attrs
        }];
        Self {
            text,
            attrs_spans,
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

    pub fn shape(&mut self, font_system: &'a FontSystem<'a>) -> &ShapeLine {
        if self.shape_opt.is_none() {
            self.shape_opt = Some(ShapeLine::new(font_system, &self.text, &self.attrs_spans));
            self.layout_opt = None;
        }
        self.shape_opt.as_ref().unwrap()
    }

    pub fn layout(&mut self, font_system: &'a FontSystem<'a>, font_size: i32, width: i32) -> &[LayoutLine] {
        if self.layout_opt.is_none() {
            let mut layout = Vec::new();
            let shape = self.shape(font_system);
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
    font_system: &'a FontSystem<'a>,
    attrs: Attrs<'a>,
    pub lines: Vec<TextBufferLine<'a>>,
    metrics: TextMetrics,
    width: i32,
    height: i32,
    scroll: i32,
    cursor: TextCursor,
    select_opt: Option<TextCursor>,
    pub cursor_moved: bool,
    pub redraw: bool,
}

impl<'a> TextBuffer<'a> {
    pub fn new(
        font_system: &'a FontSystem<'a>,
        attrs: Attrs<'a>,
        metrics: TextMetrics,
    ) -> Self {
        let mut buffer = Self {
            font_system,
            attrs,
            lines: Vec::new(),
            metrics,
            width: 0,
            height: 0,
            scroll: 0,
            cursor: TextCursor::default(),
            select_opt: None,
            cursor_moved: false,
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
                self.font_system,
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

    /// Shape lines until cursor, also scrolling to include cursor in view
    pub fn shape_until_cursor(&mut self) {
        let instant = Instant::now();

        let mut reshaped = 0;
        let mut layout_i = 0;
        for (line_i, line) in self.lines.iter_mut().enumerate() {
            if line_i > self.cursor.line {
                break;
            }

            if line.shape_opt.is_none() {
                reshaped += 1;
            }
            let layout = line.layout(
                self.font_system,
                self.metrics.font_size,
                self.width
            );
            if line_i == self.cursor.line {
                let layout_cursor = self.layout_cursor(&self.cursor);
                layout_i += layout_cursor.layout as i32;
                break;
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

    pub fn shape_until_scroll(&mut self) {
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
                    self.font_system,
                    self.metrics.font_size,
                    self.width
                );
            }
        }

        self.redraw = true;

        let duration = instant.elapsed();
        log::debug!("relayout: {:?}", duration);
    }

    fn layout_cursor(&self, cursor: &TextCursor) -> TextLayoutCursor {
        let line = &self.lines[cursor.line];

        let layout = line.layout_opt.as_ref().unwrap(); //TODO: ensure layout is done?
        for (layout_i, layout_line) in layout.iter().enumerate() {
            for (glyph_i, glyph) in layout_line.glyphs.iter().enumerate() {
                if cursor.index == glyph.start {
                    return TextLayoutCursor::new(
                        cursor.line,
                        layout_i,
                        glyph_i
                    );
                }
            }
            match layout_line.glyphs.last() {
                Some(glyph) => {
                    if cursor.index == glyph.end {
                        return TextLayoutCursor::new(
                            cursor.line,
                            layout_i,
                            layout_line.glyphs.len()
                        );
                    }
                },
                None => {
                    return TextLayoutCursor::new(
                        cursor.line,
                        layout_i,
                        0
                    );
                }
            }
        }

        // Fall back to start of line
        //TODO: should this be the end of the line?
        TextLayoutCursor::new(
            cursor.line,
            0,
            0
        )
    }

    fn set_layout_cursor(&mut self, cursor: TextLayoutCursor) {
        let line = &mut self.lines[cursor.line];
        let layout = line.layout(
            self.font_system,
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

    pub fn attrs(&self) -> &Attrs<'a> {
        &self.attrs
    }

    /// Set attributes
    pub fn set_attrs(&mut self, attrs: Attrs<'a>) {
        if attrs != self.attrs {
            self.attrs = attrs;

            for line in self.lines.iter_mut() {
                line.reset();
                line.attrs_spans = vec![AttrsSpan {
                    start: 0,
                    end: line.text.len(),
                    attrs
                }];
            }

            self.shape_until_scroll();
        }
    }

    /// Set text of buffer
    pub fn set_text(&mut self, text: &str) {
        self.lines.clear();
        for line in text.lines() {
            self.lines.push(TextBufferLine::new(line.to_string(), self.attrs));
        }
        // Make sure there is always one line
        if self.lines.is_empty() {
            self.lines.push(TextBufferLine::new(String::new(), self.attrs));
        }

        self.scroll = 0;
        self.cursor = TextCursor::default();
        self.select_opt = None;

        self.shape_until_scroll();
    }

    /// Get the lines of the original text
    pub fn text_lines(&self) -> &[TextBufferLine] {
        &self.lines
    }

    /// Perform a [TextAction] on the buffer
    pub fn action(&mut self, action: TextAction) {
        let old_cursor = self.cursor;

        match action {
            TextAction::Previous => {
                let line = &mut self.lines[self.cursor.line];

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
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.index = self.lines[self.cursor.line].text.len();
                    self.redraw = true;
                }
            },
            TextAction::Next => {
                let line = &mut self.lines[self.cursor.line];

                if self.cursor.index < line.text.len() {
                    for (i, c) in line.text.grapheme_indices(true) {
                        if i == self.cursor.index {
                            self.cursor.index += c.len();
                            self.redraw = true;
                            break;
                        }
                    }
                } else if self.cursor.line + 1 < self.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.index = 0;
                    self.redraw = true;
                }
            },
            TextAction::Left => {
                let rtl_opt = self.lines[self.cursor.line].shape_opt.as_ref().map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(TextAction::Next);
                    } else {
                        self.action(TextAction::Previous);
                    }
                }
            },
            TextAction::Right => {
                let rtl_opt = self.lines[self.cursor.line].shape_opt.as_ref().map(|shape| shape.rtl);
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
                } else if cursor.line > 0 {
                    cursor.line -= 1;
                    cursor.layout = usize::max_value();
                }
                self.set_layout_cursor(cursor);
            },
            TextAction::Down => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.layout_cursor(&self.cursor);
                let layout_len = {
                    let line = &mut self.lines[cursor.line];
                    let layout = line.layout(
                        self.font_system,
                        self.metrics.font_size,
                        self.width
                    );
                    layout.len()
                };
                if cursor.layout + 1 < layout_len {
                    cursor.layout += 1;
                } else if cursor.line + 1 < self.lines.len() {
                    cursor.line += 1;
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
                    let line = &mut self.lines[self.cursor.line];
                    line.reset();
                    line.text.insert(self.cursor.index, character);

                    self.cursor.index += character.len_utf8();
                }
            },
            TextAction::Enter => {
                let new_line = {
                    let line = &mut self.lines[self.cursor.line];
                    line.reset();
                    line.text.split_off(self.cursor.index)
                };

                let next_line = self.cursor.line + 1;
                self.lines.insert(next_line, TextBufferLine::new(new_line, self.attrs));

                self.cursor.line = next_line;
                self.cursor.index = 0;
            },
            TextAction::Backspace => {
                if self.cursor.index > 0 {
                    let line = &mut self.lines[self.cursor.line];
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
                } else if self.cursor.line > 0 {
                    let mut line_index = self.cursor.line;
                    let old_line = self.lines.remove(line_index);
                    line_index -= 1;

                    let line = &mut self.lines[line_index];
                    line.reset();

                    self.cursor.line = line_index;
                    self.cursor.index = line.text.len();

                    line.text.push_str(&old_line.text);
                }
            },
            TextAction::Delete => {
                if self.cursor.index < self.lines[self.cursor.line].text.len() {
                    let line = &mut self.lines[self.cursor.line];
                    line.reset();

                    if let Some((i, c)) = line
                        .text
                        .grapheme_indices(true)
                        .take_while(|(i, _)| *i <= self.cursor.index)
                        .last()
                    {
                        line.text.replace_range(i..(i + c.len()), "");
                        self.cursor.index = i;
                    }
                } else if self.cursor.line + 1 < self.lines.len() {
                    let old_line = self.lines.remove(self.cursor.line + 1);

                    let line = &mut self.lines[self.cursor.line];
                    line.reset();

                    line.text.push_str(&old_line.text);
                }
            },
            TextAction::Click { x, y } => {
                self.select_opt = None;

                if let Some(new_cursor) = self.hit(x, y) {
                    if new_cursor != self.cursor {
                        self.cursor = new_cursor;
                        self.redraw = true;
                    }
                }
            },
            TextAction::Drag { x, y } => {
                if self.select_opt.is_none() {
                    self.select_opt = Some(self.cursor);
                    self.redraw = true;
                }

                if let Some(new_cursor) = self.hit(x, y) {
                    if new_cursor != self.cursor {
                        self.cursor = new_cursor;
                        self.redraw = true;
                    }
                }
            },
            TextAction::Scroll { lines } => {
                self.scroll += lines;
                self.redraw = true;

                self.shape_until_scroll();
            }
        }

        if old_cursor != self.cursor {
            self.cursor_moved = true;
        }
    }

    /// Convert x, y position to TextCursor (hit detection)
    pub fn hit(&self, x: i32, y: i32) -> Option<TextCursor> {
        let instant = Instant::now();

        let font_size = self.metrics.font_size;
        let line_height = self.metrics.line_height;

        let mut new_cursor_opt = None;

        for run in self.layout_runs() {
            let line_y = run.line_y;

            if y >= line_y - font_size
            && y < line_y - font_size + line_height
            {
                let mut new_cursor_glyph = run.glyphs.len();
                let mut new_cursor_char = 0;
                'hit: for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                    if x >= glyph.x as i32
                    && x <= (glyph.x + glyph.w) as i32
                    {
                        new_cursor_glyph = glyph_i;

                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count();
                        let mut egc_x = glyph.x;
                        let egc_w = glyph.w / (total as f32);
                        for (egc_i, egc) in cluster.grapheme_indices(true) {
                            if x >= egc_x as i32
                            && x <= (egc_x + egc_w) as i32
                            {
                                new_cursor_char = egc_i;

                                let right_half = x >= (egc_x + egc_w / 2.0) as i32;
                                if right_half != glyph.rtl {
                                    // If clicking on last half of glyph, move cursor past glyph
                                    new_cursor_char += egc.len();
                                }
                                break 'hit;
                            }
                            egc_x += egc_w;
                        }

                        let right_half = x >= (glyph.x + glyph.w / 2.0) as i32;
                        if right_half != glyph.rtl {
                            // If clicking on last half of glyph, move cursor past glyph
                            new_cursor_char = cluster.len();
                        }
                        break 'hit;
                    }
                }

                let mut new_cursor = TextCursor::new(run.line_i, 0);

                match run.glyphs.get(new_cursor_glyph) {
                    Some(glyph) => {
                        // Position at glyph
                        new_cursor.index = glyph.start + new_cursor_char;
                    },
                    None => if let Some(glyph) = run.glyphs.last() {
                        // Position at end of line
                        new_cursor.index = glyph.end;
                    },
                }

                if new_cursor != self.cursor {
                    if let Some(glyph) = run.glyphs.get(new_cursor_glyph) {
                        let font_opt = self.font_system.get_font(glyph.cache_key.font_id);
                        let text_glyph = &run.text[glyph.start..glyph.end];
                        log::debug!(
                            "{}, {}: '{}' ('{}'): '{}' ({:?})",
                            self.cursor.line,
                            self.cursor.index,
                            font_opt.as_ref().map_or("?", |font| font.info.family.as_str()),
                            font_opt.as_ref().map_or("?", |font| font.info.post_script_name.as_str()),
                            text_glyph,
                            text_glyph
                        );
                    }
                }

                new_cursor_opt = Some(new_cursor);

                break;
            }
        }

        let duration = instant.elapsed();
        log::trace!("click({}, {}): {:?}", x, y, duration);

        new_cursor_opt
    }

    /// Get the visible layout runs for rendering and other tasks
    pub fn layout_runs<'b>(&'b self) -> TextLayoutRunIter<'a, 'b> {
        TextLayoutRunIter::new(self)
    }

    /// Draw the buffer
    #[cfg(feature = "swash")]
    pub fn draw<F>(&self, cache: &mut crate::SwashCache, color: u32, mut f: F)
        where F: FnMut(i32, i32, u32, u32, u32)
    {
        let font_size = self.metrics.font_size;
        let line_height = self.metrics.line_height;

        for run in self.layout_runs() {
            let line_i = run.line_i;
            let line_y = run.line_y;

            let cursor_glyph_opt = |cursor: &TextCursor| -> Option<(usize, f32)> {
                if cursor.line == line_i {
                    for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                        if cursor.index == glyph.start {
                            return Some((glyph_i, 0.0));
                        } else if cursor.index > glyph.start && cursor.index < glyph.end {
                            // Guess x offset based on characters
                            let mut before = 0;
                            let mut total = 0;

                            let cluster = &run.text[glyph.start..glyph.end];
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
                    match run.glyphs.last() {
                        Some(glyph) => {
                            if cursor.index == glyph.end {
                                return Some((run.glyphs.len(), 0.0));
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
                let (start, end) = match select.line.cmp(&self.cursor.line) {
                    cmp::Ordering::Greater => (self.cursor, select),
                    cmp::Ordering::Less => (select, self.cursor),
                    cmp::Ordering::Equal => {
                        /* select.line == self.cursor.line */
                        if select.index < self.cursor.index {
                            (select, self.cursor)
                        } else {
                            /* select.index >= self.cursor.index */
                            (self.cursor, select)
                        }
                    }
                };

                if line_i >= start.line && line_i <= end.line {
                    let mut range_opt = None;
                    for glyph in run.glyphs.iter() {
                        // Guess x offset based on characters
                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count();
                        let mut c_x = glyph.x;
                        let c_w = glyph.w / total as f32;
                        for (i, c) in cluster.grapheme_indices(true) {
                            let c_start = glyph.start + i;
                            let c_end = glyph.start + i + c.len();
                            if (start.line != line_i || c_end > start.index)
                            && (end.line != line_i || c_start < end.index) {
                                range_opt = match range_opt.take() {
                                    Some((min, max)) => Some((
                                        cmp::min(min, c_x as i32),
                                        cmp::max(max, (c_x + c_w) as i32),
                                    )),
                                    None => Some((
                                        c_x as i32,
                                        (c_x + c_w) as i32,
                                    ))
                                };
                            } else if let Some((min, max)) = range_opt.take() {
                                f(
                                    min,
                                    line_y - font_size,
                                    cmp::max(0, max - min) as u32,
                                    line_height as u32,
                                    0x33_00_00_00 | (color & 0xFF_FF_FF)
                                );
                            }
                            c_x += c_w;
                        }
                    }

                    if let Some((mut min, mut max)) = range_opt.take() {
                        if end.line > line_i {
                            // Draw to end of line
                            if run.rtl {
                                min = 0;
                            } else {
                                max = self.width;
                            }
                        }
                        f(
                            min,
                            line_y - font_size,
                            cmp::max(0, max - min) as u32,
                            line_height as u32,
                            0x33_00_00_00 | (color & 0xFF_FF_FF)
                        );
                    }
                }
            }

            // Draw cursor
            if let Some((cursor_glyph, cursor_glyph_offset)) = cursor_glyph_opt(&self.cursor) {
                let x = match run.glyphs.get(cursor_glyph) {
                    Some(glyph) => {
                        // Start of detected glyph
                        if glyph.rtl {
                            (glyph.x + glyph.w - cursor_glyph_offset) as i32
                        } else {
                            (glyph.x + cursor_glyph_offset) as i32
                        }
                    },
                    None => match run.glyphs.last() {
                        Some(glyph) => {
                            // End of last glyph
                            if glyph.rtl {
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

            for glyph in run.glyphs.iter() {
                let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some.0,
                    None => color,
                };

                cache.with_pixels(cache_key, glyph_color, |x, y, color| {
                    f(x_int + x, line_y + y_int + y, 1, 1, color)
                });
            }
        }
    }
}
