use alloc::string::String;
use core::cmp;
use modit::{Event, Motion, Operator, Parser, WordIter};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Action, AttrsList, BorrowedWithFontSystem, Buffer, Color, Cursor, Edit, FontSystem,
    SyntaxEditor, SyntaxTheme,
};

pub use modit::{ViMode, ViParser};

#[derive(Debug)]
pub struct ViEditor<'a> {
    editor: SyntaxEditor<'a>,
    parser: ViParser,
    passthrough: bool,
    search_opt: Option<(String, bool)>,
}

impl<'a> ViEditor<'a> {
    pub fn new(editor: SyntaxEditor<'a>) -> Self {
        Self {
            editor,
            parser: ViParser::new(),
            passthrough: false,
            search_opt: None,
        }
    }

    /// Modifies the theme of the [`SyntaxEditor`], returning false if the theme is missing
    pub fn update_theme(&mut self, theme_name: &str) -> bool {
        self.editor.update_theme(theme_name)
    }

    /// Load text from a file, and also set syntax to the best option
    #[cfg(feature = "std")]
    pub fn load_text<P: AsRef<std::path::Path>>(
        &mut self,
        font_system: &mut FontSystem,
        path: P,
        attrs: crate::Attrs,
    ) -> std::io::Result<()> {
        self.editor.load_text(font_system, path, attrs)
    }

    /// Get the default background color
    pub fn background_color(&self) -> Color {
        self.editor.background_color()
    }

    /// Get the default foreground (text) color
    pub fn foreground_color(&self) -> Color {
        self.editor.foreground_color()
    }

    /// Get the current syntect theme
    pub fn theme(&self) -> &SyntaxTheme {
        self.editor.theme()
    }

    /// Set passthrough mode (true will turn off vi features)
    pub fn set_passthrough(&mut self, passthrough: bool) {
        if passthrough != self.passthrough {
            self.passthrough = passthrough;
            self.buffer_mut().set_redraw(true);
        }
    }

    /// Get current vi parser
    pub fn parser(&self) -> &ViParser {
        &self.parser
    }

    fn search(&mut self, inverted: bool) {
        let (search, mut forwards) = match &self.search_opt {
            Some(some) => some,
            None => return,
        };

        if inverted {
            forwards = !forwards;
        }

        let mut cursor = self.cursor();
        let start_line = cursor.line;
        if forwards {
            while cursor.line < self.buffer().lines.len() {
                if let Some(index) = self.buffer().lines[cursor.line]
                    .text()
                    .match_indices(search.as_str())
                    .filter_map(|(i, _)| {
                        if cursor.line != start_line || i > cursor.index {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .next()
                {
                    cursor.index = index;
                    self.set_cursor(cursor);
                    return;
                }

                cursor.line += 1;
            }
        } else {
            cursor.line += 1;
            while cursor.line > 0 {
                cursor.line -= 1;

                if let Some(index) = self.buffer().lines[cursor.line]
                    .text()
                    .rmatch_indices(search.as_str())
                    .filter_map(|(i, _)| {
                        if cursor.line != start_line || i < cursor.index {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .next()
                {
                    cursor.index = index;
                    self.set_cursor(cursor);
                    return;
                }
            }
        }
    }
}

impl<'a> Edit for ViEditor<'a> {
    fn buffer(&self) -> &Buffer {
        self.editor.buffer()
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        self.editor.buffer_mut()
    }

    fn cursor(&self) -> Cursor {
        self.editor.cursor()
    }

    fn set_cursor(&mut self, cursor: Cursor) {
        self.editor.set_cursor(cursor);
    }

    fn select_opt(&self) -> Option<Cursor> {
        self.editor.select_opt()
    }

    fn set_select_opt(&mut self, select_opt: Option<Cursor>) {
        self.editor.set_select_opt(select_opt);
    }

    fn tab_width(&self) -> usize {
        self.editor.tab_width()
    }

    fn set_tab_width(&mut self, tab_width: usize) {
        self.editor.set_tab_width(tab_width);
    }

    fn shape_as_needed(&mut self, font_system: &mut FontSystem) {
        self.editor.shape_as_needed(font_system);
    }

    fn copy_selection(&self) -> Option<String> {
        self.editor.copy_selection()
    }

    fn delete_selection(&mut self) -> bool {
        self.editor.delete_selection()
    }

    fn insert_string(&mut self, data: &str, attrs_list: Option<AttrsList>) {
        self.editor.insert_string(data, attrs_list);
    }

    fn action(&mut self, font_system: &mut FontSystem, action: Action) {
        let editor = &mut self.editor;
        log::info!("Action {:?}", action);
        let c = match action {
            Action::Escape => modit::ESCAPE,
            Action::Insert(c) => c,
            Action::Enter => modit::ENTER,
            Action::Backspace => modit::BACKSPACE,
            Action::Delete => modit::DELETE,
            _ => return editor.action(font_system, action),
        };
        //TODO: redraw on parser mode change
        self.parser.parse(c, false, |event| {
            log::info!("  Event {:?}", event);
            let action = match event {
                Event::Redraw => {
                    editor.buffer_mut().set_redraw(true);
                    return;
                }
                Event::Backspace => Action::Backspace,
                Event::Delete => Action::Delete,
                Event::Escape => Action::Escape,
                Event::Insert(c) => Action::Insert(c),
                Event::NewLine => Action::Enter,
                Event::Paste => {
                    log::info!("TODO");
                    return;
                }
                Event::Undo => {
                    log::info!("TODO");
                    return;
                }
                Event::Cmd(count, operator, motion, text_object_opt) => {
                    let start = editor.cursor();

                    for _ in 0..count {
                        let action = match motion {
                            Motion::Down => Action::Down,
                            Motion::End => Action::End,
                            Motion::GotoLine(line) => Action::GotoLine(line.saturating_sub(1)),
                            Motion::GotoEof => {
                                Action::GotoLine(editor.buffer().lines.len().saturating_sub(1))
                            }
                            Motion::Home => Action::Home,
                            Motion::Left => Action::Left,
                            Motion::NextChar(find_c) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                let text = buffer.lines[cursor.line].text();
                                if cursor.index < text.len() {
                                    match text[cursor.index..]
                                        .char_indices()
                                        .filter(|&(i, c)| i > 0 && c == find_c)
                                        .next()
                                    {
                                        Some((i, _)) => {
                                            cursor.index += i;
                                            editor.set_cursor(cursor);
                                        }
                                        None => {}
                                    }
                                }
                                continue;
                            }
                            Motion::NextCharTill(find_c) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                let text = buffer.lines[cursor.line].text();
                                if cursor.index < text.len() {
                                    let mut last_i = 0;
                                    for (i, c) in text[cursor.index..].char_indices() {
                                        if last_i > 0 && c == find_c {
                                            cursor.index += last_i;
                                            editor.set_cursor(cursor);
                                            break;
                                        } else {
                                            last_i = i;
                                        }
                                    }
                                }
                                continue;
                            }
                            Motion::NextWordEnd(word) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                loop {
                                    let text = buffer.lines[cursor.line].text();
                                    if cursor.index < text.len() {
                                        cursor.index = WordIter::new(text, word)
                                            .map(|(i, w)| {
                                                i + w
                                                    .char_indices()
                                                    .last()
                                                    .map(|(i, _)| i)
                                                    .unwrap_or(0)
                                            })
                                            .find(|&i| i > cursor.index)
                                            .unwrap_or(text.len());
                                        if cursor.index == text.len() {
                                            // Try again, searching next line
                                            continue;
                                        }
                                    } else if cursor.line + 1 < buffer.lines.len() {
                                        // Go to next line and rerun loop
                                        cursor.line += 1;
                                        cursor.index = 0;
                                        continue;
                                    }
                                    break;
                                }
                                editor.set_cursor(cursor);
                                continue;
                            }
                            Motion::NextWordStart(word) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                loop {
                                    let text = buffer.lines[cursor.line].text();
                                    if cursor.index < text.len() {
                                        cursor.index = WordIter::new(text, word)
                                            .map(|(i, _)| i)
                                            .find(|&i| i > cursor.index)
                                            .unwrap_or(text.len());
                                        if cursor.index == text.len() {
                                            // Try again, searching next line
                                            continue;
                                        }
                                    } else if cursor.line + 1 < buffer.lines.len() {
                                        // Go to next line and rerun loop
                                        cursor.line += 1;
                                        cursor.index = 0;
                                        continue;
                                    }
                                    break;
                                }
                                editor.set_cursor(cursor);
                                continue;
                            }
                            Motion::PreviousChar(find_c) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                let text = buffer.lines[cursor.line].text();
                                if cursor.index > 0 {
                                    match text[..cursor.index]
                                        .char_indices()
                                        .filter(|&(_, c)| c == find_c)
                                        .last()
                                    {
                                        Some((i, _)) => {
                                            cursor.index = i;
                                            editor.set_cursor(cursor);
                                        }
                                        None => {}
                                    }
                                }
                                continue;
                            }
                            Motion::PreviousCharTill(find_c) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                let text = buffer.lines[cursor.line].text();
                                if cursor.index > 0 {
                                    match text[..cursor.index]
                                        .char_indices()
                                        .filter_map(|(i, c)| {
                                            if c == find_c {
                                                let end = i + c.len_utf8();
                                                if end < cursor.index {
                                                    return Some(end);
                                                }
                                            }
                                            None
                                        })
                                        .last()
                                    {
                                        Some(i) => {
                                            cursor.index = i;
                                            editor.set_cursor(cursor);
                                        }
                                        None => {}
                                    }
                                }
                                continue;
                            }
                            Motion::PreviousWordEnd(word) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                loop {
                                    let text = buffer.lines[cursor.line].text();
                                    if cursor.index > 0 {
                                        cursor.index = WordIter::new(text, word)
                                            .map(|(i, w)| {
                                                i + w
                                                    .char_indices()
                                                    .last()
                                                    .map(|(i, _)| i)
                                                    .unwrap_or(0)
                                            })
                                            .filter(|&i| i < cursor.index)
                                            .last()
                                            .unwrap_or(0);
                                        if cursor.index == 0 {
                                            // Try again, searching previous line
                                            continue;
                                        }
                                    } else if cursor.line > 0 {
                                        // Go to previous line and rerun loop
                                        cursor.line -= 1;
                                        cursor.index = buffer.lines[cursor.line].text().len();
                                        continue;
                                    }
                                    break;
                                }
                                editor.set_cursor(cursor);
                                continue;
                            }
                            Motion::PreviousWordStart(word) => {
                                let mut cursor = editor.cursor();
                                let buffer = editor.buffer_mut();
                                loop {
                                    let text = buffer.lines[cursor.line].text();
                                    if cursor.index > 0 {
                                        cursor.index = WordIter::new(text, word)
                                            .map(|(i, _)| i)
                                            .filter(|&i| i < cursor.index)
                                            .last()
                                            .unwrap_or(0);
                                        if cursor.index == 0 {
                                            // Try again, searching previous line
                                            continue;
                                        }
                                    } else if cursor.line > 0 {
                                        // Go to previous line and rerun loop
                                        cursor.line -= 1;
                                        cursor.index = buffer.lines[cursor.line].text().len();
                                        continue;
                                    }
                                    break;
                                }
                                editor.set_cursor(cursor);
                                continue;
                            }
                            Motion::Right => Action::Right,
                            Motion::SoftHome => Action::SoftHome,
                            Motion::Up => Action::Up,
                            _ => {
                                log::info!("TODO");
                                break;
                            }
                        };
                        editor.action(font_system, action);
                    }

                    let end = editor.cursor();

                    println!("start {:?}, end {:?}", start, end);
                    return;
                }
            };
            editor.action(font_system, action);
        });
    }

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
        let size = self.buffer().size();
        f(0, 0, size.0 as u32, size.1 as u32, self.background_color());

        let font_size = self.buffer().metrics().font_size;
        let line_height = self.buffer().metrics().line_height;

        for run in self.buffer().layout_runs() {
            let line_i = run.line_i;
            let line_y = run.line_y;
            let line_top = run.line_top;

            let cursor_glyph_opt = |cursor: &Cursor| -> Option<(usize, f32, f32)> {
                //TODO: better calculation of width
                let default_width = font_size / 2.0;
                if cursor.line == line_i {
                    for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                        if cursor.index >= glyph.start && cursor.index < glyph.end {
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

                            let width = glyph.w / (total as f32);
                            let offset = (before as f32) * width;
                            return Some((glyph_i, offset, width));
                        }
                    }
                    match run.glyphs.last() {
                        Some(glyph) => {
                            if cursor.index == glyph.end {
                                return Some((run.glyphs.len(), 0.0, default_width));
                            }
                        }
                        None => {
                            return Some((0, 0.0, default_width));
                        }
                    }
                }
                None
            };

            // Highlight selection (TODO: HIGHLIGHT COLOR!)
            if let Some(select) = self.select_opt() {
                let (start, end) = match select.line.cmp(&self.cursor().line) {
                    cmp::Ordering::Greater => (self.cursor(), select),
                    cmp::Ordering::Less => (select, self.cursor()),
                    cmp::Ordering::Equal => {
                        /* select.line == self.cursor.line */
                        if select.index < self.cursor().index {
                            (select, self.cursor())
                        } else {
                            /* select.index >= self.cursor.index */
                            (self.cursor(), select)
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
                        range_opt = Some((0, self.buffer().size().0 as i32));
                    }

                    if let Some((mut min, mut max)) = range_opt.take() {
                        if end.line > line_i {
                            // Draw to end of line
                            if run.rtl {
                                min = 0;
                            } else {
                                max = self.buffer().size().0 as i32;
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
            if let Some((cursor_glyph, cursor_glyph_offset, cursor_glyph_width)) =
                cursor_glyph_opt(&self.cursor())
            {
                let block_cursor = if self.passthrough {
                    false
                } else {
                    match self.parser.mode {
                        ViMode::Insert | ViMode::Replace => false,
                        _ => true, /*TODO: determine block cursor in other modes*/
                    }
                };

                let (start_x, end_x) = match run.glyphs.get(cursor_glyph) {
                    Some(glyph) => {
                        // Start of detected glyph
                        if glyph.level.is_rtl() {
                            (
                                (glyph.x + glyph.w - cursor_glyph_offset) as i32,
                                (glyph.x + glyph.w - cursor_glyph_offset - cursor_glyph_width)
                                    as i32,
                            )
                        } else {
                            (
                                (glyph.x + cursor_glyph_offset) as i32,
                                (glyph.x + cursor_glyph_offset + cursor_glyph_width) as i32,
                            )
                        }
                    }
                    None => match run.glyphs.last() {
                        Some(glyph) => {
                            // End of last glyph
                            if glyph.level.is_rtl() {
                                (glyph.x as i32, (glyph.x - cursor_glyph_width) as i32)
                            } else {
                                (
                                    (glyph.x + glyph.w) as i32,
                                    (glyph.x + glyph.w + cursor_glyph_width) as i32,
                                )
                            }
                        }
                        None => {
                            // Start of empty line
                            (0, cursor_glyph_width as i32)
                        }
                    },
                };

                if block_cursor {
                    let left_x = cmp::min(start_x, end_x);
                    let right_x = cmp::max(start_x, end_x);
                    f(
                        left_x,
                        line_top as i32,
                        (right_x - left_x) as u32,
                        line_height as u32,
                        Color::rgba(color.r(), color.g(), color.b(), 0x33),
                    );
                } else {
                    f(start_x, line_top as i32, 1, line_height as u32, color);
                }
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

impl<'a, 'b> BorrowedWithFontSystem<'b, ViEditor<'a>> {
    /// Load text from a file, and also set syntax to the best option
    #[cfg(feature = "std")]
    pub fn load_text<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
        attrs: crate::Attrs,
    ) -> std::io::Result<()> {
        self.inner.load_text(self.font_system, path, attrs)
    }
}
