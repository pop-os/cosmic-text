use cosmic_text::{
    Action,
    Buffer,
    Color,
    Cursor,
    Edit,
    SyntaxEditor,
};
use std::{
    io,
    path::Path,
};

enum Mode {
    Normal,
    Insert,
    Command,
    Search,
    SearchBackwards,
}

pub struct Vi<'a> {
    editor: SyntaxEditor<'a>,
    mode: Mode,
}

impl<'a> Vi<'a> {
    pub fn new(editor: SyntaxEditor<'a>) -> Self {
        Self {
            editor,
            mode: Mode::Normal,
        }
    }

    /// Load text from a file, and also set syntax to the best option
    pub fn load_text<P: AsRef<Path>>(&mut self, path: P, attrs: crate::Attrs<'a>) -> io::Result<()> {
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

impl<'a> Edit<'a> for Vi<'a> {
    fn buffer(&self) -> &Buffer<'a> {
        self.editor.buffer()
    }

    fn buffer_mut(&mut self) -> &mut Buffer<'a> {
        self.editor.buffer_mut()
    }

    fn cursor(&self) -> Cursor {
        self.editor.cursor()
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

    fn action(&mut self, action: Action) {
        match self.mode {
            Mode::Normal => match action {
                Action::Insert(c) => match c {
                    // Enter insert mode after cursor
                    'a' => {
                        self.editor.action(Action::Right);
                        self.mode = Mode::Insert;
                    },
                    // Enter insert mode at end of line
                    'A' => {
                        self.editor.action(Action::End);
                        self.mode = Mode::Insert;
                    },
                    // Enter insert mode at cursor
                    'i' => {
                        self.mode = Mode::Insert;
                    },
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
                    },
                    // Create line before and enter insert mode
                    'O' => {
                        self.editor.action(Action::Home);
                        self.editor.action(Action::Enter);
                        self.editor.shape_as_needed(); // TODO: do not require this?
                        self.editor.action(Action::Up);
                        self.mode = Mode::Insert;
                    },
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
                    },
                    // Enter search mode
                    '/' => {
                        self.mode = Mode::Search;
                    },
                    // Enter search backwards mode
                    '?' => {
                        self.mode = Mode::SearchBackwards;
                    },
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
                },
                _ => self.editor.action(action),
            },
            _ => {
                //TODO: other modes
                self.mode = Mode::Normal;
            },
        }
    }

    fn draw<F>(&self, cache: &mut crate::SwashCache, color: Color, f: F)
        where F: FnMut(i32, i32, u32, u32, Color)
    {
        //TODO: block cursor in normal mode?
        self.editor.draw(cache, color, f);
    }
}
