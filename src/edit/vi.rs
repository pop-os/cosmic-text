use alloc::string::String;
use core::cmp;
use unicode_segmentation::UnicodeSegmentation;

use crate::{Action, AttrsList, Buffer, Color, Cursor, Edit, SyntaxEditor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Command,
    Search,
    SearchBackwards,
}

pub struct ViEditor<'a> {
    editor: SyntaxEditor<'a>,
    mode: Mode,
}

impl<'a> ViEditor<'a> {
    pub fn new(editor: SyntaxEditor<'a>) -> Self {
        Self {
            editor,
            mode: Mode::Normal,
        }
    }

    /// Load text from a file, and also set syntax to the best option
    #[cfg(feature = "std")]
    pub fn load_text<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
        attrs: crate::Attrs<'a>,
    ) -> std::io::Result<()> {
        self.editor.load_text(path, attrs)
    }

    /// Get the default background color
    pub fn background_color(&self) -> Color {
        self.editor.background_color()
    }

    /// Get the default foreground (text) color
    pub fn foreground_color(&self) -> Color {
        self.editor.foreground_color()
    }
}

impl<'a> Edit<'a> for ViEditor<'a> {
    fn buffer(&self) -> &Buffer<'a> {
        self.editor.buffer()
    }

    fn buffer_mut(&mut self) -> &mut Buffer<'a> {
        self.editor.buffer_mut()
    }

    fn cursor(&self) -> Cursor {
        self.editor.cursor()
    }

    fn select_opt(&self) -> Option<Cursor> {
        self.editor.select_opt()
    }

    fn set_select_opt(&mut self, select_opt: Option<Cursor>) {
        self.editor.set_select_opt(select_opt);
    }

    fn shape_as_needed(&mut self) {
        self.editor.shape_as_needed()
    }

    fn copy_selection(&mut self) -> Option<String> {
        self.editor.copy_selection()
    }

    fn delete_selection(&mut self) -> bool {
        self.editor.delete_selection()
    }

    fn insert_string(&mut self, data: &str, attrs_list: Option<AttrsList>) {
        self.editor.insert_string(data, attrs_list);
    }

    fn action(&mut self, action: Action) {
        let old_mode = self.mode;

        match self.mode {
            Mode::Normal => match action {
                Action::Insert(c) => match c {
                    // Enter insert mode after cursor
                    'a' => {
                        self.editor.action(Action::Right);
                        self.mode = Mode::Insert;
                    }
                    // Enter insert mode at end of line
                    'A' => {
                        self.editor.action(Action::End);
                        self.mode = Mode::Insert;
                    }
                    // Change mode
                    'c' => {
                        if self.editor.select_opt().is_some() {
                            self.editor.action(Action::Delete);
                            self.mode = Mode::Insert;
                        } else {
                            //TODO: change to next cursor movement
                        }
                    }
                    // Delete mode
                    'd' => {
                        if self.editor.select_opt().is_some() {
                            self.editor.action(Action::Delete);
                        } else {
                            //TODO: delete to next cursor movement
                        }
                    }
                    // Enter insert mode at cursor
                    'i' => {
                        self.mode = Mode::Insert;
                    }
                    // Enter insert mode at start of line
                    'I' => {
                        //TODO: soft home, skip whitespace
                        self.editor.action(Action::Home);
                        self.mode = Mode::Insert;
                    }
                    // Create line after and enter insert mode
                    'o' => {
                        self.editor.action(Action::End);
                        self.editor.action(Action::Enter);
                        self.mode = Mode::Insert;
                    }
                    // Create line before and enter insert mode
                    'O' => {
                        self.editor.action(Action::Home);
                        self.editor.action(Action::Enter);
                        self.editor.shape_as_needed(); // TODO: do not require this?
                        self.editor.action(Action::Up);
                        self.mode = Mode::Insert;
                    }
                    // Left
                    'h' => self.editor.action(Action::Left),
                    // Top of screen
                    //TODO: 'H' => self.editor.action(Action::ScreenHigh),
                    // Down
                    'j' => self.editor.action(Action::Down),
                    // Up
                    'k' => self.editor.action(Action::Up),
                    // Right
                    'l' => self.editor.action(Action::Right),
                    // Bottom of screen
                    //TODO: 'L' => self.editor.action(Action::ScreenLow),
                    // Middle of screen
                    //TODO: 'M' => self.editor.action(Action::ScreenMiddle),
                    // Enter visual mode
                    'v' => {
                        if self.editor.select_opt().is_some() {
                            self.editor.set_select_opt(None);
                        } else {
                            self.editor.set_select_opt(Some(self.editor.cursor()));
                        }
                    }
                    // Enter line visual mode
                    'V' => {
                        if self.editor.select_opt().is_some() {
                            self.editor.set_select_opt(None);
                        } else {
                            self.editor.action(Action::Home);
                            self.editor.set_select_opt(Some(self.editor.cursor()));
                            //TODO: set cursor_x_opt to max
                            self.editor.action(Action::End);
                        }
                    }
                    // Remove character at cursor
                    'x' => self.editor.action(Action::Delete),
                    // Remove character before cursor
                    'X' => self.editor.action(Action::Backspace),
                    // Go to start of line
                    '0' => self.editor.action(Action::Home),
                    // Go to end of line
                    '$' => self.editor.action(Action::End),
                    // Go to start of line after whitespace
                    //TODO: implement this
                    '^' => self.editor.action(Action::Home),
                    // Enter command mode
                    ':' => {
                        self.mode = Mode::Command;
                    }
                    // Enter search mode
                    '/' => {
                        self.mode = Mode::Search;
                    }
                    // Enter search backwards mode
                    '?' => {
                        self.mode = Mode::SearchBackwards;
                    }
                    _ => (),
                },
                _ => self.editor.action(action),
            },
            Mode::Insert => match action {
                Action::Escape => {
                    let cursor = self.cursor();
                    let layout_cursor = self.buffer().layout_cursor(&cursor);
                    if layout_cursor.glyph > 0 {
                        self.editor.action(Action::Left);
                    }
                    self.mode = Mode::Normal;
                }
                _ => self.editor.action(action),
            },
            _ => {
                //TODO: other modes
                self.mode = Mode::Normal;
            }
        }

        if self.mode != old_mode {
            self.buffer_mut().set_redraw(true);
        }
    }

    #[cfg(feature = "swash")]
    fn draw<F>(&self, cache: &mut crate::SwashCache, color: Color, mut f: F)
    where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        let font_size = self.buffer().metrics().font_size;
        let line_height = self.buffer().metrics().line_height;

        for run in self.buffer().layout_runs() {
            let line_i = run.line_i;
            let line_y = run.line_y;

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
                                    (line_y - font_size) as i32,
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
                            (line_y - font_size) as i32,
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
                let block_cursor = match self.mode {
                    Mode::Normal => true,
                    Mode::Insert => false,
                    _ => true, /*TODO: determine block cursor in other modes*/
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
                        (line_y - font_size) as i32,
                        (right_x - left_x) as u32,
                        line_height as u32,
                        Color::rgba(color.r(), color.g(), color.b(), 0x33),
                    );
                } else {
                    f(
                        start_x,
                        (line_y - font_size) as i32,
                        1,
                        line_height as u32,
                        color,
                    );
                }
            }

            for glyph in run.glyphs.iter() {
                let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => color,
                };

                cache.with_pixels(cache_key, glyph_color, |x, y, color| {
                    f(x_int + x, line_y as i32 + y_int + y, 1, 1, color);
                });
            }
        }
    }
}
