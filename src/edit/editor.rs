// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
use core::{
    cmp::{self, Ordering},
    iter::once,
};
use unicode_segmentation::UnicodeSegmentation;

#[cfg(feature = "swash")]
use crate::Color;
use crate::{
    Action, Affinity, AttrsList, Buffer, BufferLine, Change, ChangeItem, Cursor, Edit, FontSystem,
    LayoutCursor, Shaping,
};

/// A wrapper of [`Buffer`] for easy editing
#[derive(Debug)]
pub struct Editor {
    buffer: Buffer,
    cursor: Cursor,
    cursor_x_opt: Option<i32>,
    select_opt: Option<Cursor>,
    cursor_moved: bool,
    tab_width: u16,
    change: Option<Change>,
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
            tab_width: 4,
            change: None,
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

    fn delete_range(&mut self, start: Cursor, end: Cursor) {
        // Collect removed data for change tracking
        let mut change_text = String::new();

        // Delete the selection from the last line
        let end_line_opt = if end.line > start.line {
            // Get part of line after selection
            let after = self.buffer.lines[end.line].split_off(end.index);

            // Remove end line
            let removed = self.buffer.lines.remove(end.line);
            change_text.insert_str(0, removed.text());

            Some(after)
        } else {
            None
        };

        // Delete interior lines (in reverse for safety)
        for line_i in (start.line + 1..end.line).rev() {
            let removed = self.buffer.lines.remove(line_i);
            change_text.insert_str(0, removed.text());
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
            let removed = self.buffer.lines[start.line].split_off(start.index);
            change_text.insert_str(0, removed.text());

            // Re-add part of line after selection
            if let Some(after) = after_opt {
                self.buffer.lines[start.line].append(after);
            }

            // Re-add valid parts of end line
            if let Some(end_line) = end_line_opt {
                self.buffer.lines[start.line].append(end_line);
            }
        }

        if let Some(ref mut change) = self.change {
            let item = ChangeItem {
                start,
                end,
                text: change_text,
                insert: false,
            };
            change.items.push(item);
        }
    }

    fn insert_at(
        &mut self,
        mut cursor: Cursor,
        data: &str,
        attrs_list: Option<AttrsList>,
    ) -> Cursor {
        let mut remaining_split_len = data.len();
        if remaining_split_len == 0 {
            return cursor;
        }

        // Save cursor for change tracking
        let start = cursor;

        let line: &mut BufferLine = &mut self.buffer.lines[cursor.line];
        let insert_line = cursor.line + 1;

        // Collect text after insertion as a line
        let after: BufferLine = line.split_off(cursor.index);
        let after_len = after.text().len();

        // Collect attributes
        let mut final_attrs = attrs_list.unwrap_or_else(|| {
            AttrsList::new(line.attrs_list().get_span(cursor.index.saturating_sub(1)))
        });

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
            cursor.line += 1;
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
            cursor.line += 1;
        }

        assert_eq!(remaining_split_len, 0);

        // Append the text after insertion
        cursor.index = self.buffer.lines[cursor.line].text().len() - after_len;

        if let Some(ref mut change) = self.change {
            let item = ChangeItem {
                start,
                end: cursor,
                text: data.to_string(),
                insert: true,
            };
            change.items.push(item);
        }

        cursor
    }
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
        if self.cursor != cursor {
            self.cursor = cursor;
            self.cursor_moved = true;
            self.cursor_x_opt = None;
            self.buffer.set_redraw(true);
        }
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

    fn tab_width(&self) -> u16 {
        self.tab_width
    }

    fn set_tab_width(&mut self, tab_width: u16) {
        // A tab width of 0 is not allowed
        if tab_width == 0 {
            return;
        }
        if self.tab_width != tab_width {
            self.tab_width = tab_width;
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

    fn copy_selection(&self) -> Option<String> {
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

        // Delete from start to end of selection
        self.delete_range(start, end);

        true
    }

    fn insert_string(&mut self, data: &str, attrs_list: Option<AttrsList>) {
        self.delete_selection();
        let new_cursor = self.insert_at(self.cursor, data, attrs_list);
        self.set_cursor(new_cursor);
    }

    fn apply_change(&mut self, change: &Change) -> bool {
        // Cannot apply changes if there is a pending change
        match self.change.take() {
            Some(pending) => {
                if !pending.items.is_empty() {
                    //TODO: is this a good idea?
                    log::warn!("pending change caused apply_change to be ignored!");
                    self.change = Some(pending);
                    return false;
                }
            }
            None => {}
        }

        for item in change.items.iter() {
            //TODO: edit cursor if needed?
            if item.insert {
                self.cursor = self.insert_at(item.start, &item.text, None);
            } else {
                self.cursor = item.start;
                self.delete_range(item.start, item.end);
            }
        }
        true
    }

    fn start_change(&mut self) {
        if self.change.is_none() {
            self.change = Some(Change::default());
        }
    }

    fn finish_change(&mut self) -> Option<Change> {
        self.change.take()
    }

    fn action(&mut self, font_system: &mut FontSystem, action: Action) {
        let old_cursor = self.cursor;

        match action {
            Action::Previous => {
                let line = &self.buffer.lines[self.cursor.line];
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
                let line = &self.buffer.lines[self.cursor.line];
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
                let rtl_opt = self
                    .buffer
                    .line_shape(font_system, self.cursor.line)
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
                let rtl_opt = self
                    .buffer
                    .line_shape(font_system, self.cursor.line)
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
            Action::SoftHome => {
                let line = &self.buffer.lines[self.cursor.line];
                self.cursor.index = line
                    .text()
                    .char_indices()
                    .filter_map(|(i, c)| if c.is_whitespace() { None } else { Some(i) })
                    .next()
                    .unwrap_or(0);
                self.buffer.set_redraw(true);
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
            Action::Enter => {
                self.insert_string("\n", None);

                // Ensure line is properly shaped and laid out (for potential immediate commands)
                self.buffer.line_layout(font_system, self.cursor.line);
            }
            Action::Backspace => {
                if self.delete_selection() {
                    // Deleted selection
                } else {
                    // Save current cursor as end
                    let end = self.cursor;

                    if self.cursor.index > 0 {
                        // Move cursor to previous character index
                        let line = &self.buffer.lines[self.cursor.line];
                        self.cursor.index = line.text()[..self.cursor.index]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i);
                    } else if self.cursor.line > 0 {
                        // Move cursor to previous line
                        self.cursor.line -= 1;
                        let line = &self.buffer.lines[self.cursor.line];
                        self.cursor.index = line.text().len();
                    }

                    if self.cursor != end {
                        // Delete range
                        self.delete_range(self.cursor, end);
                    }
                }
            }
            Action::Delete => {
                if self.delete_selection() {
                    // Deleted selection
                } else {
                    // Save current cursor as end
                    let mut end = self.cursor;

                    if self.cursor.index < self.buffer.lines[self.cursor.line].text().len() {
                        let line = &self.buffer.lines[self.cursor.line];

                        let range_opt = line
                            .text()
                            .grapheme_indices(true)
                            .take_while(|(i, _)| *i <= self.cursor.index)
                            .last()
                            .map(|(i, c)| i..(i + c.len()));

                        if let Some(range) = range_opt {
                            self.cursor.index = range.start;
                            end.index = range.end;
                        }
                    } else if self.cursor.line + 1 < self.buffer.lines.len() {
                        end.line += 1;
                        end.index = 0;
                    }

                    if self.cursor != end {
                        self.delete_range(self.cursor, end);
                    }
                }
            }
            Action::Indent => {
                // Get start and end of selection
                let (start, end) = match self.select_opt {
                    Some(select) => match select.line.cmp(&self.cursor.line) {
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
                    },
                    None => (self.cursor, self.cursor),
                };

                // For every line in selection
                let tab_width: usize = self.tab_width.into();
                for line_i in start.line..=end.line {
                    // Determine indexes of last indent and first character after whitespace
                    let mut after_whitespace;
                    let mut required_indent = 0;
                    {
                        let line = &self.buffer.lines[line_i];
                        let text = line.text();
                        // Default to end of line if no non-whitespace found
                        after_whitespace = text.len();
                        for (count, (index, c)) in text.char_indices().enumerate() {
                            if !c.is_whitespace() {
                                after_whitespace = index;
                                required_indent = tab_width - (count % tab_width);
                                break;
                            }
                        }
                    }

                    // No indent required (not possible?)
                    if required_indent == 0 {
                        required_indent = tab_width;
                    }

                    self.insert_at(
                        Cursor::new(line_i, after_whitespace),
                        &" ".repeat(required_indent),
                        None,
                    );

                    // Adjust cursor
                    if self.cursor.line == line_i {
                        //TODO: should we be forcing cursor index to current indent location?
                        if self.cursor.index < after_whitespace {
                            self.cursor.index = after_whitespace;
                        }
                        self.cursor.index += required_indent;
                    }

                    // Adjust selection
                    if let Some(ref mut select) = self.select_opt {
                        if select.line == line_i && select.index >= after_whitespace {
                            select.index += required_indent;
                        }
                    }

                    // Request redraw
                    self.buffer.set_redraw(true);
                }
            }
            Action::Unindent => {
                // Get start and end of selection
                let (start, end) = match self.select_opt {
                    Some(select) => match select.line.cmp(&self.cursor.line) {
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
                    },
                    None => (self.cursor, self.cursor),
                };

                // For every line in selection
                let tab_width: usize = self.tab_width.into();
                for line_i in start.line..=end.line {
                    // Determine indexes of last indent and first character after whitespace
                    let mut last_indent = 0;
                    let mut after_whitespace;
                    {
                        let line = &self.buffer.lines[line_i];
                        let text = line.text();
                        // Default to end of line if no non-whitespace found
                        after_whitespace = text.len();
                        for (count, (index, c)) in text.char_indices().enumerate() {
                            if !c.is_whitespace() {
                                after_whitespace = index;
                                break;
                            }
                            if count % tab_width == 0 {
                                last_indent = index;
                            }
                        }
                    }

                    // No de-indent required
                    if last_indent == after_whitespace {
                        continue;
                    }

                    // Delete one indent
                    self.delete_range(
                        Cursor::new(line_i, last_indent),
                        Cursor::new(line_i, after_whitespace),
                    );

                    // Adjust cursor
                    if self.cursor.line == line_i && self.cursor.index > last_indent {
                        self.cursor.index -= after_whitespace - last_indent;
                    }

                    // Adjust selection
                    if let Some(ref mut select) = self.select_opt {
                        if select.line == line_i && select.index > last_indent {
                            select.index -= after_whitespace - last_indent;
                        }
                    }

                    // Request redraw
                    self.buffer.set_redraw(true);
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
                let line = &self.buffer.lines[self.cursor.line];
                if self.cursor.index > 0 {
                    self.cursor.index = line
                        .text()
                        .unicode_word_indices()
                        .rev()
                        .map(|(i, _)| i)
                        .find(|&i| i < self.cursor.index)
                        .unwrap_or(0);

                    self.buffer.set_redraw(true);
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.index = self.buffer.lines[self.cursor.line].text().len();
                    self.buffer.set_redraw(true);
                }
                self.cursor_x_opt = None;
            }
            Action::NextWord => {
                let line = &self.buffer.lines[self.cursor.line];
                if self.cursor.index < line.text().len() {
                    self.cursor.index = line
                        .text()
                        .unicode_word_indices()
                        .map(|(i, word)| i + word.len())
                        .find(|&i| i > self.cursor.index)
                        .unwrap_or(line.text().len());

                    self.buffer.set_redraw(true);
                } else if self.cursor.line + 1 < self.buffer.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.index = 0;
                    self.buffer.set_redraw(true);
                }
                self.cursor_x_opt = None;
            }
            Action::LeftWord => {
                let rtl_opt = self
                    .buffer
                    .line_shape(font_system, self.cursor.line)
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
                let rtl_opt = self
                    .buffer
                    .line_shape(font_system, self.cursor.line)
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
            Action::GotoLine(line) => {
                let mut cursor = self.buffer.layout_cursor(&self.cursor);
                cursor.line = line;
                self.set_layout_cursor(font_system, cursor);
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
                        }
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
                                && (end.line != line_i || c_start < end.index)
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
                                    Color::rgba(color.r(), color.g(), color.b(), 0x33),
                                );
                            }
                            c_x += c_w;
                        }
                    }

                    if run.glyphs.is_empty() && end.line > line_i {
                        // Highlight all of internal empty lines
                        range_opt = Some((0, self.buffer.size().0 as i32));
                    }

                    if let Some((mut min, mut max)) = range_opt.take() {
                        if end.line > line_i {
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
                            Color::rgba(color.r(), color.g(), color.b(), 0x33),
                        );
                    }
                }
            }

            // Draw cursor
            if let Some((cursor_glyph, cursor_glyph_offset)) = cursor_glyph_opt(&self.cursor) {
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

                f(
                    x,
                    line_top as i32,
                    1,
                    line_height as u32,
                    self.cursor.color.unwrap_or(color),
                );
            }

            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0., 0.), 1.0);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => color,
                };

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
}
