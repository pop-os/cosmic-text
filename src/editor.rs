// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cmp;
use unicode_segmentation::UnicodeSegmentation;

use crate::{AttrsList, Buffer, BufferLine, Color, Cursor, LayoutCursor};

/// An action to perform on an [Editor]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Move cursor to previous character ([Self::Left] in LTR, [Self::Right] in RTL)
    Previous,
    /// Move cursor to next character ([Self::Right] in LTR, [Self::Left] in RTL)
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

/// A wrapper of [Buffer] for easy editing
pub struct Editor<'a> {
    pub buffer: Buffer<'a>,
    cursor: Cursor,
    cursor_x_opt: Option<i32>,
    select_opt: Option<Cursor>,
    cursor_moved: bool,
}

impl<'a> Editor<'a> {
    /// Create a new [Editor] with the provided [Buffer]
    pub fn new(buffer: Buffer<'a>) -> Self {
        Self {
            buffer,
            cursor: Cursor::default(),
            cursor_x_opt: None,
            select_opt: None,
            cursor_moved: false,
        }
    }

    /// Shape lines until scroll, after adjusting scroll if the cursor moved
    pub fn shape_as_needed(&mut self) {
        if self.cursor_moved {
            self.buffer.shape_until_cursor(self.cursor);
            self.cursor_moved = false;
        } else {
            self.buffer.shape_until_scroll();
        }
    }

    fn set_layout_cursor(&mut self, cursor: LayoutCursor) {
        let layout = self.buffer.line_layout(cursor.line).unwrap();

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
            self.buffer.redraw = true;
        }
    }

    /// Get the current cursor position
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Copy selection
    pub fn copy_selection(&mut self) -> Option<String> {
        let select = self.select_opt?;

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

        let mut selection = String::new();
        // Take the selection from the first line
        {
            // Add selected part of line to string
            if start.line == end.line {
                selection.push_str(&self.buffer.lines[start.line].text()[start.index..end.index]);
            } else {
                selection.push_str(&self.buffer.lines[start.line].text()[start.index..]);
                selection.push('\n');
            }
        }

        // Take the selection from all interior lines (if they exist)
        for line_i in start.line + 1..end.line {
            selection.push_str(self.buffer.lines[line_i].text());
            selection.push('\n');
        }

        // Take the selection from the last line
        if end.line > start.line {
            // Add selected part of line to string
            selection.push_str(&self.buffer.lines[end.line].text()[..end.index]);
        }

        Some(selection)
    }

    /// Delete selection, adjusting cursor and returning true if there was a selection
    // Helper function for backspace, delete, insert, and enter when there is a selection
    pub fn delete_selection(&mut self) -> bool {
        let select = match self.select_opt.take() {
            Some(some) => some,
            None => return false,
        };

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

        // Reset cursor to start of selection
        self.cursor = start;

        // Delete the selection from the last line
        let end_line_opt = if end.line > start.line {
            // Get part of line after selection
            let after = self.buffer.lines[end.line].split_off(end.index);

            self.buffer.lines.remove(end.line);

            Some(after)
        } else {
            None
        };

        // Delete interior lines (in reverse for safety)
        for line_i in (start.line + 1..end.line).rev() {
            self.buffer.lines.remove(line_i);
        }

        // Delete the selection from the first line
        {
            // Get part after selection if start line is also end line
            let after_opt = if start.line == end.line {
                Some(self.buffer.lines[start.line].split_off(end.index))
            } else {
                None
            };

            // Delete selected part of line
            self.buffer.lines[start.line].split_off(start.index);

            // Re-add part of line after selection
            if let Some(after) = after_opt {
                self.buffer.lines[start.line].append(after);
            }

            // Re-add valid parts of end line
            if let Some(end_line) = end_line_opt {
                self.buffer.lines[start.line].append(end_line);
            }
        }

        true
    }

    /// Perform a [Action] on the editor
    pub fn action(&mut self, action: Action) {
        let old_cursor = self.cursor;

        match action {
            Action::Previous => {
                let line = &mut self.buffer.lines[self.cursor.line];
                if self.cursor.index > 0 {
                    // Find previous character index
                    let mut prev_index = 0;
                    for (i, _) in line.text().grapheme_indices(true) {
                        if i < self.cursor.index {
                            prev_index = i;
                        } else {
                            break;
                        }
                    }

                    self.cursor.index = prev_index;
                    self.buffer.redraw = true;
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.index = self.buffer.lines[self.cursor.line].text().len();
                    self.buffer.redraw = true;
                }
                self.cursor_x_opt = None;
            },
            Action::Next => {
                let line = &mut self.buffer.lines[self.cursor.line];
                if self.cursor.index < line.text().len() {
                    for (i, c) in line.text().grapheme_indices(true) {
                        if i == self.cursor.index {
                            self.cursor.index += c.len();
                            self.buffer.redraw = true;
                            break;
                        }
                    }
                } else if self.cursor.line + 1 < self.buffer.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.index = 0;
                    self.buffer.redraw = true;
                }
                self.cursor_x_opt = None;
            },
            Action::Left => {
                let rtl_opt = self.buffer.lines[self.cursor.line].shape_opt().as_ref().map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(Action::Next);
                    } else {
                        self.action(Action::Previous);
                    }
                }
            },
            Action::Right => {
                let rtl_opt = self.buffer.lines[self.cursor.line].shape_opt().as_ref().map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(Action::Previous);
                    } else {
                        self.action(Action::Next);
                    }
                }
            },
            Action::Up => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.buffer.layout_cursor(&self.cursor);

                if self.cursor_x_opt.is_none() {
                    self.cursor_x_opt = Some(
                        cursor.glyph as i32 //TODO: glyph x position
                    );
                }

                if cursor.layout > 0 {
                    cursor.layout -= 1;
                } else if cursor.line > 0 {
                    cursor.line -= 1;
                    cursor.layout = usize::max_value();
                }

                if let Some(cursor_x) = self.cursor_x_opt {
                    cursor.glyph = cursor_x as usize; //TODO: glyph x position
                }

                self.set_layout_cursor(cursor);
            },
            Action::Down => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.buffer.layout_cursor(&self.cursor);

                let layout_len = self.buffer.line_layout(cursor.line).unwrap().len();

                if self.cursor_x_opt.is_none() {
                    self.cursor_x_opt = Some(
                        cursor.glyph as i32 //TODO: glyph x position
                    );
                }

                if cursor.layout + 1 < layout_len {
                    cursor.layout += 1;
                } else if cursor.line + 1 < self.buffer.lines.len() {
                    cursor.line += 1;
                    cursor.layout = 0;
                }

                if let Some(cursor_x) = self.cursor_x_opt {
                    cursor.glyph = cursor_x as usize; //TODO: glyph x position
                }

                self.set_layout_cursor(cursor);
            },
            Action::Home => {
                let mut cursor = self.buffer.layout_cursor(&self.cursor);
                cursor.glyph = 0;
                self.set_layout_cursor(cursor);
                self.cursor_x_opt = None;
            },
            Action::End => {
                let mut cursor = self.buffer.layout_cursor(&self.cursor);
                cursor.glyph = usize::max_value();
                self.set_layout_cursor(cursor);
                self.cursor_x_opt = None;
            }
            Action::PageUp => {
                //TODO: move cursor
                let mut scroll = self.buffer.scroll();
                scroll -= self.buffer.visible_lines();
                self.buffer.set_scroll(scroll);
            },
            Action::PageDown => {
                //TODO: move cursor
                let mut scroll = self.buffer.scroll();
                scroll += self.buffer.visible_lines();
                self.buffer.set_scroll(scroll);
            },
            Action::Insert(character) => {
                if character.is_control()
                && !['\t', '\u{92}'].contains(&character)
                {
                    // Filter out special chars (except for tab), use Action instead
                    log::debug!("Refusing to insert control character {:?}", character);
                } else {
                    self.delete_selection();

                    let line = &mut self.buffer.lines[self.cursor.line];

                    // Collect text after insertion as a line
                    let after = line.split_off(self.cursor.index);

                    // Append the inserted text
                    line.append(BufferLine::new(
                        character.to_string(),
                        AttrsList::new(line.attrs_list().defaults() /*TODO: provide attrs?*/)
                    ));

                    // Append the text after insertion
                    line.append(after);

                    self.cursor.index += character.len_utf8();
                }
            },
            Action::Enter => {
                self.delete_selection();

                let new_line = self.buffer.lines[self.cursor.line].split_off(self.cursor.index);

                self.cursor.line += 1;
                self.cursor.index = 0;

                self.buffer.lines.insert(self.cursor.line, new_line);
            },
            Action::Backspace => {
                if self.delete_selection() {
                    // Deleted selection
                } else if self.cursor.index > 0 {
                    let line = &mut self.buffer.lines[self.cursor.line];

                    // Get text line after cursor
                    let after = line.split_off(self.cursor.index);

                    // Find previous character index
                    let mut prev_index = 0;
                    for (i, _) in line.text().char_indices() {
                        if i < self.cursor.index {
                            prev_index = i;
                        } else {
                            break;
                        }
                    }

                    self.cursor.index = prev_index;

                    // Remove character
                    line.split_off(self.cursor.index);

                    // Add text after cursor
                    line.append(after);
                } else if self.cursor.line > 0 {
                    let mut line_index = self.cursor.line;
                    let old_line = self.buffer.lines.remove(line_index);
                    line_index -= 1;

                    let line = &mut self.buffer.lines[line_index];

                    self.cursor.line = line_index;
                    self.cursor.index = line.text().len();

                    line.append(old_line);
                }
            },
            Action::Delete => {
                if self.delete_selection() {
                    // Deleted selection
                } else if self.cursor.index < self.buffer.lines[self.cursor.line].text().len() {
                    let line = &mut self.buffer.lines[self.cursor.line];

                    let range_opt = line
                        .text()
                        .grapheme_indices(true)
                        .take_while(|(i, _)| *i <= self.cursor.index)
                        .last()
                        .map(|(i, c)| {
                            i..(i + c.len())
                        });

                    if let Some(range) = range_opt {
                        self.cursor.index = range.start;

                        // Get text after deleted EGC
                        let after = line.split_off(range.end);

                        // Delete EGC
                        line.split_off(range.start);

                        // Add text after deleted EGC
                        line.append(after);
                    }
                } else if self.cursor.line + 1 < self.buffer.lines.len() {
                    let old_line = self.buffer.lines.remove(self.cursor.line + 1);
                    self.buffer.lines[self.cursor.line].append(old_line);
                }
            },
            Action::Click { x, y } => {
                self.select_opt = None;

                if let Some(new_cursor) = self.buffer.hit(x, y) {
                    if new_cursor != self.cursor {
                        self.cursor = new_cursor;
                        self.buffer.redraw = true;
                    }
                }
            },
            Action::Drag { x, y } => {
                if self.select_opt.is_none() {
                    self.select_opt = Some(self.cursor);
                    self.buffer.redraw = true;
                }

                if let Some(new_cursor) = self.buffer.hit(x, y) {
                    if new_cursor != self.cursor {
                        self.cursor = new_cursor;
                        self.buffer.redraw = true;
                    }
                }
            },
            Action::Scroll { lines } => {
                let mut scroll = self.buffer.scroll();
                scroll += lines;
                self.buffer.set_scroll(scroll);
            }
        }

        if old_cursor != self.cursor {
            self.cursor_moved = true;

            /*TODO
            if let Some(glyph) = run.glyphs.get(new_cursor_glyph) {
                let font_opt = self.buffer.font_system().get_font(glyph.cache_key.font_id);
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
            */
        }
    }

    /// Draw the editor
    #[cfg(feature = "swash")]
    pub fn draw<F>(&self, cache: &mut crate::SwashCache, color: Color, mut f: F)
        where F: FnMut(i32, i32, u32, u32, Color)
    {
        let font_size = self.buffer.metrics().font_size;
        let line_height = self.buffer.metrics().line_height;

        for run in self.buffer.layout_runs() {
            let line_i = run.line_i;
            let line_y = run.line_y;

            let cursor_glyph_opt = |cursor: &Cursor| -> Option<(usize, f32)> {
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
                                    Color::rgba(color.r(), color.g(), color.b(), 0x33)
                                );
                            }
                            c_x += c_w;
                        }
                    }

                    if run.glyphs.is_empty() && end.line > line_i{
                        // Highlight all of internal empty lines
                        range_opt = Some((0, self.buffer.size().0));
                    }

                    if let Some((mut min, mut max)) = range_opt.take() {
                        if end.line > line_i {
                            // Draw to end of line
                            if run.rtl {
                                min = 0;
                            } else {
                                max = self.buffer.size().0;
                            }
                        }
                        f(
                            min,
                            line_y - font_size,
                            cmp::max(0, max - min) as u32,
                            line_height as u32,
                            Color::rgba(color.r(), color.g(), color.b(), 0x33)
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
                    Some(some) => some,
                    None => color,
                };

                cache.with_pixels(cache_key, glyph_color, |x, y, color| {
                    f(x_int + x, line_y + y_int + y, 1, 1, color)
                });
            }
        }
    }
}
