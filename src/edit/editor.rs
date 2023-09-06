// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::{
    cmp::{self, Ordering},
    iter::once,
    ops::Range,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Action, Affinity, AttrsList, Buffer, BufferLine, Color, Cursor, Edit, FontSystem, LayoutCursor,
    LayoutRun, Shaping,
};

/// A wrapper of [`Buffer`] for easy editing
#[derive(Debug)]
pub struct Editor {
    buffer: Buffer,
    cursor: Cursor,
    cursor_x_opt: Option<i32>,
    select_opt: Option<Cursor>,
    cursor_moved: bool,
    // A preedit was specified with non-empty text but with empty cursor,
    // indicating that the cursor should be hidden
    has_preedit_without_cursor: bool,
    // Set with `set_cursor_hidden`
    cursor_hidden_by_setting: bool,
    selection_color: Option<Color>,
    selected_text_color: Option<Color>,
}

impl Editor {
    /// Create a new [`Editor`] with the provided [`Buffer`]
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            cursor: Cursor::default(),
            cursor_x_opt: None,
            select_opt: None,
            cursor_moved: false,
            has_preedit_without_cursor: false,
            cursor_hidden_by_setting: false,
            selection_color: None,
            selected_text_color: None,
        }
    }

    fn set_layout_cursor(&mut self, font_system: &mut FontSystem, cursor: LayoutCursor) {
        let layout = self
            .buffer
            .line_layout(font_system, cursor.line)
            .expect("layout not found");

        let layout_line = match layout.get(cursor.layout) {
            Some(some) => some,
            None => match layout.last() {
                Some(some) => some,
                None => todo!("layout cursor in line with no layouts"),
            },
        };

        let (new_index, new_affinity) = match layout_line.glyphs.get(cursor.glyph) {
            Some(glyph) => (glyph.start, Affinity::After),
            None => match layout_line.glyphs.last() {
                Some(glyph) => (glyph.end, Affinity::Before),
                //TODO: is this correct?
                None => (0, Affinity::After),
            },
        };

        if self.cursor.line != cursor.line
            || self.cursor.index != new_index
            || self.cursor.affinity != new_affinity
        {
            self.cursor.line = cursor.line;
            self.cursor.index = new_index;
            self.cursor.affinity = new_affinity;
            self.buffer.set_redraw(true);
        }
    }
}

fn cursor_glyph_opt(cursor: &Cursor, run: &LayoutRun) -> Option<(usize, f32)> {
    if cursor.line == run.line_i {
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
            }
            None => {
                return Some((0, 0.0));
            }
        }
    }
    None
}

fn cursor_position(cursor: &Cursor, run: &LayoutRun) -> Option<(i32, i32)> {
    let (cursor_glyph, cursor_glyph_offset) = cursor_glyph_opt(cursor, run)?;
    let x = match run.glyphs.get(cursor_glyph) {
        Some(glyph) => {
            // Start of detected glyph
            if glyph.level.is_rtl() {
                (glyph.x + glyph.w - cursor_glyph_offset) as i32
            } else {
                (glyph.x + cursor_glyph_offset) as i32
            }
        }
        None => match run.glyphs.last() {
            Some(glyph) => {
                // End of last glyph
                if glyph.level.is_rtl() {
                    glyph.x as i32
                } else {
                    (glyph.x + glyph.w) as i32
                }
            }
            None => {
                // Start of empty line
                0
            }
        },
    };

    Some((x, run.line_top as i32))
}

impl Edit for Editor {
    fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    fn cursor(&self) -> Cursor {
        self.cursor
    }

    fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
        self.buffer.set_redraw(true);
    }

    fn set_cursor_hidden(&mut self, hidden: bool) {
        self.cursor_hidden_by_setting = hidden;
        self.buffer.set_redraw(true);
    }

    fn select_opt(&self) -> Option<Cursor> {
        self.select_opt
    }

    fn set_select_opt(&mut self, select_opt: Option<Cursor>) {
        if self.select_opt != select_opt {
            self.select_opt = select_opt;
            self.buffer.set_redraw(true);
        }
    }

    fn shape_as_needed(&mut self, font_system: &mut FontSystem) {
        if self.cursor_moved {
            self.buffer.shape_until_cursor(font_system, self.cursor);
            self.cursor_moved = false;
        } else {
            self.buffer.shape_until_scroll(font_system);
        }
    }

    fn copy_selection(&mut self) -> Option<String> {
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

    fn delete_selection(&mut self) -> bool {
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

            // Remove end line
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

    fn insert_string(&mut self, data: &str, attrs_list: Option<AttrsList>) {
        self.delete_selection();
        let mut remaining_split_len = data.len();
        if remaining_split_len == 0 {
            return;
        }

        let line: &mut BufferLine = &mut self.buffer.lines[self.cursor.line];
        let insert_line = self.cursor.line + 1;

        // Collect text after insertion as a line
        let after: BufferLine = line.split_off(self.cursor.index);
        let after_len = after.text().len();

        // Collect attributes
        let mut final_attrs = attrs_list
            .unwrap_or_else(|| AttrsList::new(line.attrs_list().get_span(line.text().len())));

        // Append the inserted text, line by line
        // we want to see a blank entry if the string ends with a newline
        let addendum = once("").filter(|_| data.ends_with('\n'));
        let mut lines_iter = data.split_inclusive('\n').chain(addendum);
        if let Some(data_line) = lines_iter.next() {
            let mut these_attrs = final_attrs.split_off(data_line.len());
            remaining_split_len -= data_line.len();
            core::mem::swap(&mut these_attrs, &mut final_attrs);
            line.append(BufferLine::new(
                data_line
                    .strip_suffix(char::is_control)
                    .unwrap_or(data_line),
                these_attrs,
                Shaping::Advanced,
            ));
        } else {
            panic!("str::lines() did not yield any elements");
        }
        if let Some(data_line) = lines_iter.next_back() {
            remaining_split_len -= data_line.len();
            let mut tmp = BufferLine::new(
                data_line
                    .strip_suffix(char::is_control)
                    .unwrap_or(data_line),
                final_attrs.split_off(remaining_split_len),
                Shaping::Advanced,
            );
            tmp.append(after);
            self.buffer.lines.insert(insert_line, tmp);
            self.cursor.line += 1;
        } else {
            line.append(after);
        }
        for data_line in lines_iter.rev() {
            remaining_split_len -= data_line.len();
            let tmp = BufferLine::new(
                data_line
                    .strip_suffix(char::is_control)
                    .unwrap_or(data_line),
                final_attrs.split_off(remaining_split_len),
                Shaping::Advanced,
            );
            self.buffer.lines.insert(insert_line, tmp);
            self.cursor.line += 1;
        }

        assert_eq!(remaining_split_len, 0);

        // Append the text after insertion
        self.cursor.index = self.buffer.lines[self.cursor.line].text().len() - after_len;
    }

    fn preedit_range(&self) -> Option<Range<usize>> {
        self.buffer().lines[self.cursor.line].preedit_range()
    }

    fn preedit_text(&self) -> Option<&str> {
        self.buffer().lines[self.cursor.line].preedit_text()
    }

    fn action(&mut self, font_system: &mut FontSystem, action: Action) {
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
                    self.cursor.affinity = Affinity::After;
                    self.buffer.set_redraw(true);
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.index = self.buffer.lines[self.cursor.line].text().len();
                    self.cursor.affinity = Affinity::After;
                    self.buffer.set_redraw(true);
                }
                self.cursor_x_opt = None;
            }
            Action::Next => {
                let line = &mut self.buffer.lines[self.cursor.line];
                if self.cursor.index < line.text().len() {
                    for (i, c) in line.text().grapheme_indices(true) {
                        if i == self.cursor.index {
                            self.cursor.index += c.len();
                            self.cursor.affinity = Affinity::Before;
                            self.buffer.set_redraw(true);
                            break;
                        }
                    }
                } else if self.cursor.line + 1 < self.buffer.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.index = 0;
                    self.cursor.affinity = Affinity::Before;
                    self.buffer.set_redraw(true);
                }
                self.cursor_x_opt = None;
            }
            Action::Left => {
                let rtl_opt = self.buffer.lines[self.cursor.line]
                    .shape_opt()
                    .as_ref()
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(font_system, Action::Next);
                    } else {
                        self.action(font_system, Action::Previous);
                    }
                }
            }
            Action::Right => {
                let rtl_opt = self.buffer.lines[self.cursor.line]
                    .shape_opt()
                    .as_ref()
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(font_system, Action::Previous);
                    } else {
                        self.action(font_system, Action::Next);
                    }
                }
            }
            Action::Up => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.buffer.layout_cursor(&self.cursor);

                if self.cursor_x_opt.is_none() {
                    self.cursor_x_opt = Some(
                        cursor.glyph as i32, //TODO: glyph x position
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

                self.set_layout_cursor(font_system, cursor);
            }
            Action::Down => {
                //TODO: make this preserve X as best as possible!
                let mut cursor = self.buffer.layout_cursor(&self.cursor);

                let layout_len = self
                    .buffer
                    .line_layout(font_system, cursor.line)
                    .expect("layout not found")
                    .len();

                if self.cursor_x_opt.is_none() {
                    self.cursor_x_opt = Some(
                        cursor.glyph as i32, //TODO: glyph x position
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

                self.set_layout_cursor(font_system, cursor);
            }
            Action::Home => {
                let mut cursor = self.buffer.layout_cursor(&self.cursor);
                cursor.glyph = 0;
                self.set_layout_cursor(font_system, cursor);
                self.cursor_x_opt = None;
            }
            Action::End => {
                let mut cursor = self.buffer.layout_cursor(&self.cursor);
                cursor.glyph = usize::max_value();
                self.set_layout_cursor(font_system, cursor);
                self.cursor_x_opt = None;
            }
            Action::ParagraphStart => {
                self.cursor.index = 0;
                self.cursor_x_opt = None;
                self.buffer.set_redraw(true);
            }
            Action::ParagraphEnd => {
                self.cursor.index = self.buffer.lines[self.cursor.line].text().len();
                self.cursor_x_opt = None;
                self.buffer.set_redraw(true);
            }
            Action::PageUp => {
                self.action(font_system, Action::Vertical(-self.buffer.size().1 as i32));
            }
            Action::PageDown => {
                self.action(font_system, Action::Vertical(self.buffer.size().1 as i32));
            }
            Action::Vertical(px) => {
                // TODO more efficient
                let lines = px / self.buffer.metrics().line_height as i32;
                match lines.cmp(&0) {
                    Ordering::Less => {
                        for _ in 0..-lines {
                            self.action(font_system, Action::Up);
                        }
                    }
                    Ordering::Greater => {
                        for _ in 0..lines {
                            self.action(font_system, Action::Down);
                        }
                    }
                    Ordering::Equal => {}
                }
            }
            Action::Escape => {
                if self.select_opt.take().is_some() {
                    self.buffer.set_redraw(true);
                }
            }
            Action::Insert(character) => {
                if character.is_control() && !['\t', '\n', '\u{92}'].contains(&character) {
                    // Filter out special chars (except for tab), use Action instead
                    log::debug!("Refusing to insert control character {:?}", character);
                } else if character == '\n' {
                    self.action(font_system, Action::Enter);
                } else {
                    let mut str_buf = [0u8; 8];
                    let str_ref = character.encode_utf8(&mut str_buf);
                    self.insert_string(str_ref, None);
                }
            }
            Action::SetPreedit {
                preedit,
                cursor,
                attrs,
            } => {
                self.select_opt = None;
                let line: &mut BufferLine = &mut self.buffer.lines[self.cursor.line];
                if let Some(range) = line.preedit_range() {
                    let end = line.split_off(range.end);
                    line.split_off(range.start);
                    line.append(end);
                    self.cursor.index = range.start;
                }
                if !preedit.is_empty() {
                    let new_attrs = if let Some(attrs) = attrs {
                        AttrsList::new(attrs.as_attrs().preedit(true))
                    } else {
                        let attrs_at_cursor = line.attrs_list().get_span(self.cursor.index);
                        AttrsList::new(
                            attrs_at_cursor
                                .preedit(true)
                                .color(Color::rgb(128, 128, 128)),
                        )
                    };
                    self.insert_string(&preedit, Some(new_attrs));
                    if let Some((start, end)) = cursor {
                        let end_delta = preedit.len().saturating_sub(end);
                        self.cursor.index = self.cursor.index.saturating_sub(end_delta);
                        if start != end {
                            let start_delta = preedit.len().saturating_sub(start);
                            let mut select = self.cursor;
                            select.index = select.index.saturating_sub(start_delta);
                            self.select_opt = Some(select);
                        }
                    }
                }
                self.has_preedit_without_cursor = !preedit.is_empty() && cursor.is_none();
                self.buffer.set_redraw(true);
            }
            Action::Enter => {
                self.delete_selection();

                let new_line = self.buffer.lines[self.cursor.line].split_off(self.cursor.index);

                self.cursor.line += 1;
                self.cursor.index = 0;

                self.buffer.lines.insert(self.cursor.line, new_line);
            }
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
            }
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
                        .map(|(i, c)| i..(i + c.len()));

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
            }
            Action::Click { x, y } => {
                self.select_opt = None;

                if let Some(new_cursor) = self.buffer.hit(x as f32, y as f32) {
                    if new_cursor != self.cursor {
                        let color = self.cursor.color;
                        self.cursor = new_cursor;
                        self.cursor.color = color;
                        self.buffer.set_redraw(true);
                    }
                }
            }
            Action::Drag { x, y } => {
                if self.select_opt.is_none() {
                    self.select_opt = Some(self.cursor);
                    self.buffer.set_redraw(true);
                }

                if let Some(new_cursor) = self.buffer.hit(x as f32, y as f32) {
                    if new_cursor != self.cursor {
                        let color = self.cursor.color;
                        self.cursor = new_cursor;
                        self.cursor.color = color;
                        self.buffer.set_redraw(true);
                    }
                }
            }
            Action::Scroll { lines } => {
                let mut scroll = self.buffer.scroll();
                scroll += lines;
                self.buffer.set_scroll(scroll);
            }
            Action::PreviousWord => {
                let line: &mut BufferLine = &mut self.buffer.lines[self.cursor.line];
                if self.cursor.index > 0 {
                    let mut prev_index = 0;
                    for (i, _) in line.text().unicode_word_indices() {
                        if i < self.cursor.index {
                            prev_index = i;
                        } else {
                            break;
                        }
                    }

                    self.cursor.index = prev_index;
                    self.buffer.set_redraw(true);
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.index = self.buffer.lines[self.cursor.line].text().len();
                    self.buffer.set_redraw(true);
                }
                self.cursor_x_opt = None;
            }
            Action::NextWord => {
                let line: &mut BufferLine = &mut self.buffer.lines[self.cursor.line];
                if self.cursor.index < line.text().len() {
                    for (i, word) in line.text().unicode_word_indices() {
                        let i = i + word.len();
                        if i > self.cursor.index {
                            self.cursor.index = i;
                            self.buffer.set_redraw(true);
                            break;
                        }
                    }
                } else if self.cursor.line + 1 < self.buffer.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.index = 0;
                    self.buffer.set_redraw(true);
                }
                self.cursor_x_opt = None;
            }
            Action::LeftWord => {
                let rtl_opt = self.buffer.lines[self.cursor.line]
                    .shape_opt()
                    .as_ref()
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(font_system, Action::NextWord);
                    } else {
                        self.action(font_system, Action::PreviousWord);
                    }
                }
            }
            Action::RightWord => {
                let rtl_opt = self.buffer.lines[self.cursor.line]
                    .shape_opt()
                    .as_ref()
                    .map(|shape| shape.rtl);
                if let Some(rtl) = rtl_opt {
                    if rtl {
                        self.action(font_system, Action::PreviousWord);
                    } else {
                        self.action(font_system, Action::NextWord);
                    }
                }
            }
            Action::BufferStart => {
                self.cursor.line = 0;
                self.cursor.index = 0;
                self.cursor_x_opt = None;
            }
            Action::BufferEnd => {
                self.cursor.line = self.buffer.lines.len() - 1;
                self.cursor.index = self.buffer.lines[self.cursor.line].text().len();
                self.cursor_x_opt = None;
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
    fn draw<F>(
        &self,
        font_system: &mut FontSystem,
        cache: &mut crate::SwashCache,
        color: Color,
        mut f: F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        let line_height = self.buffer.metrics().line_height;

        for run in self.buffer.layout_runs() {
            let line_i = run.line_i;
            let line_y = run.line_y;
            let line_top = run.line_top;

            let (selection_start, selection_end) = if let Some(select) = self.select_opt {
                match select.line.cmp(&self.cursor.line) {
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
                }
            } else {
                (self.cursor, self.cursor)
            };

            // Highlight selection
            if selection_start != selection_end {
                let color = self
                    .selection_color
                    .unwrap_or_else(|| Color::rgba(color.r(), color.g(), color.b(), 0x33));

                if line_i >= selection_start.line && line_i <= selection_end.line {
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
                            if (selection_start.line != line_i || c_end > selection_start.index)
                                && (selection_end.line != line_i || c_start < selection_end.index)
                            {
                                range_opt = match range_opt.take() {
                                    Some((min, max)) => Some((
                                        cmp::min(min, c_x as i32),
                                        cmp::max(max, (c_x + c_w) as i32),
                                    )),
                                    None => Some((c_x as i32, (c_x + c_w) as i32)),
                                };
                            } else if let Some((min, max)) = range_opt.take() {
                                f(
                                    min,
                                    line_top as i32,
                                    cmp::max(0, max - min) as u32,
                                    line_height as u32,
                                    color,
                                );
                            }
                            c_x += c_w;
                        }
                    }

                    if run.glyphs.is_empty() && selection_end.line > line_i {
                        // Highlight all of internal empty lines
                        range_opt = Some((0, self.buffer.size().0 as i32));
                    }

                    if let Some((mut min, mut max)) = range_opt.take() {
                        if selection_end.line > line_i {
                            // Draw to end of line
                            if run.rtl {
                                min = 0;
                            } else {
                                max = self.buffer.size().0 as i32;
                            }
                        }

                        f(
                            min,
                            line_top as i32,
                            cmp::max(0, max - min) as u32,
                            line_height as u32,
                            color,
                        );
                    }
                }
            }

            // Draw cursor
            let cursor_hidden = self.cursor_hidden_by_setting
                || self.has_preedit_without_cursor
                || self.has_selection();
            if !cursor_hidden {
                if let Some((x, y)) = cursor_position(&self.cursor, &run) {
                    f(
                        x,
                        y,
                        1,
                        line_height as u32,
                        self.cursor.color.unwrap_or(color),
                    );
                }
            }

            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0., 0.), 1.0);

                let mut glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => color,
                };
                if let Some(color) = self.selected_text_color {
                    let is_selected = line_i >= selection_start.line
                        && line_i <= selection_end.line
                        && (selection_start.line != line_i || glyph.end > selection_start.index)
                        && (selection_end.line != line_i || glyph.start < selection_end.index);

                    if is_selected {
                        glyph_color = color;
                    }
                }

                cache.with_pixels(
                    font_system,
                    physical_glyph.cache_key,
                    glyph_color,
                    |x, y, color| {
                        f(
                            physical_glyph.x + x,
                            line_y as i32 + physical_glyph.y + y,
                            1,
                            1,
                            color,
                        );
                    },
                );
            }
        }
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        self.buffer
            .layout_runs()
            .find_map(|run| cursor_position(&self.cursor, &run))
    }

    fn set_selection_color(&mut self, color: Option<Color>) {
        self.selection_color = color;
        self.buffer.set_redraw(true);
    }

    fn set_selected_text_color(&mut self, color: Option<Color>) {
        self.selected_text_color = color;
        self.buffer.set_redraw(true);
    }
}
