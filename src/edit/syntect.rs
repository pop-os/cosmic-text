#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{fs, io, path::Path};
use syntect::highlighting::{
    FontStyle, HighlightState, Highlighter, RangedHighlightIterator, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use crate::{
    Action, AttrsList, BorrowedWithFontSystem, Buffer, Color, Cursor, Edit, Editor, FontSystem,
    Shaping, Style, Weight, Wrap,
};

#[derive(Debug)]
pub struct SyntaxSystem {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl SyntaxSystem {
    /// Create a new [`SyntaxSystem`]
    pub fn new() -> Self {
        Self {
            //TODO: store newlines in buffer
            syntax_set: SyntaxSet::load_defaults_nonewlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

/// A wrapper of [`Editor`] with syntax highlighting provided by [`SyntaxSystem`]
#[derive(Debug)]
pub struct SyntaxEditor<'a> {
    editor: Editor,
    syntax_system: &'a SyntaxSystem,
    syntax: &'a SyntaxReference,
    theme: &'a Theme,
    highlighter: Highlighter<'a>,
    syntax_cache: Vec<(ParseState, HighlightState)>,
}

impl<'a> SyntaxEditor<'a> {
    /// Create a new [`SyntaxEditor`] with the provided [`Buffer`], [`SyntaxSystem`], and theme name.
    ///
    /// A good default theme name is "base16-eighties.dark".
    ///
    /// Returns None if theme not found
    pub fn new(buffer: Buffer, syntax_system: &'a SyntaxSystem, theme_name: &str) -> Option<Self> {
        let editor = Editor::new(buffer);
        let syntax = syntax_system.syntax_set.find_syntax_plain_text();
        let theme = syntax_system.theme_set.themes.get(theme_name)?;
        let highlighter = Highlighter::new(theme);

        Some(Self {
            editor,
            syntax_system,
            syntax,
            theme,
            highlighter,
            syntax_cache: Vec::new(),
        })
    }

    /// Modifies the theme of the [`SyntaxEditor`], returning false if the theme is missing
    pub fn update_theme(&mut self, theme_name: &str) -> bool {
        if let Some(theme) = self.syntax_system.theme_set.themes.get(theme_name) {
            self.theme = theme;
            self.highlighter = Highlighter::new(theme);
            self.syntax_cache.clear();

            true
        } else {
            false
        }
    }

    /// Load text from a file, and also set syntax to the best option
    ///
    /// ## Errors
    ///
    /// Returns an [`io::Error`] if reading the file fails
    #[cfg(feature = "std")]
    pub fn load_text<P: AsRef<Path>>(
        &mut self,
        font_system: &mut FontSystem,
        path: P,
        attrs: crate::Attrs,
    ) -> io::Result<()> {
        let path = path.as_ref();

        let text = fs::read_to_string(path)?;
        self.editor
            .buffer_mut()
            .set_text(font_system, &text, attrs, Shaping::Advanced);

        //TODO: re-use text
        self.syntax = match self.syntax_system.syntax_set.find_syntax_for_file(path) {
            Ok(Some(some)) => some,
            Ok(None) => {
                log::warn!("no syntax found for {:?}", path);
                self.syntax_system.syntax_set.find_syntax_plain_text()
            }
            Err(err) => {
                log::warn!("failed to determine syntax for {:?}: {:?}", path, err);
                self.syntax_system.syntax_set.find_syntax_plain_text()
            }
        };

        // Clear syntax cache
        self.syntax_cache.clear();

        Ok(())
    }

    /// Get the default background color
    pub fn background_color(&self) -> Color {
        if let Some(background) = self.theme.settings.background {
            Color::rgba(background.r, background.g, background.b, background.a)
        } else {
            Color::rgb(0, 0, 0)
        }
    }

    /// Get the default foreground (text) color
    pub fn foreground_color(&self) -> Color {
        if let Some(foreground) = self.theme.settings.foreground {
            Color::rgba(foreground.r, foreground.g, foreground.b, foreground.a)
        } else {
            Color::rgb(0xFF, 0xFF, 0xFF)
        }
    }
}

impl<'a> Edit for SyntaxEditor<'a> {
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

    fn shape_as_needed(&mut self, font_system: &mut FontSystem) {
        #[cfg(feature = "std")]
        let now = std::time::Instant::now();

        let buffer = self.editor.buffer_mut();

        let mut highlighted = 0;
        for line_i in 0..buffer.lines.len() {
            let line = &mut buffer.lines[line_i];
            if !line.is_reset() && line_i < self.syntax_cache.len() {
                continue;
            }
            highlighted += 1;

            let (mut parse_state, mut highlight_state) =
                if line_i > 0 && line_i <= self.syntax_cache.len() {
                    self.syntax_cache[line_i - 1].clone()
                } else {
                    (
                        ParseState::new(self.syntax),
                        HighlightState::new(&self.highlighter, ScopeStack::new()),
                    )
                };

            let ops = parse_state
                .parse_line(line.text(), &self.syntax_system.syntax_set)
                .expect("failed to parse syntax");
            let ranges = RangedHighlightIterator::new(
                &mut highlight_state,
                &ops,
                line.text(),
                &self.highlighter,
            );

            let attrs = line.attrs_list().defaults();
            let mut attrs_list = AttrsList::new(attrs);
            for (style, _, range) in ranges {
                attrs_list.add_span(
                    range,
                    attrs
                        .color(Color::rgba(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                            style.foreground.a,
                        ))
                        //TODO: background
                        .style(if style.font_style.contains(FontStyle::ITALIC) {
                            Style::Italic
                        } else {
                            Style::Normal
                        })
                        .weight(if style.font_style.contains(FontStyle::BOLD) {
                            Weight::BOLD
                        } else {
                            Weight::NORMAL
                        }), //TODO: underline
                );
            }

            // Update line attributes. This operation only resets if the line changes
            line.set_attrs_list(attrs_list);
            line.set_wrap(Wrap::Word);

            //TODO: efficiently do syntax highlighting without having to shape whole buffer
            buffer.line_shape(font_system, line_i);

            let cache_item = (parse_state.clone(), highlight_state.clone());
            if line_i < self.syntax_cache.len() {
                if self.syntax_cache[line_i] != cache_item {
                    self.syntax_cache[line_i] = cache_item;
                    if line_i + 1 < buffer.lines.len() {
                        buffer.lines[line_i + 1].reset();
                    }
                }
            } else {
                self.syntax_cache.push(cache_item);
            }
        }

        if highlighted > 0 {
            buffer.set_redraw(true);
            #[cfg(feature = "std")]
            log::debug!(
                "Syntax highlighted {} lines in {:?}",
                highlighted,
                now.elapsed()
            );
        }

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
        self.editor.action(font_system, action);
    }

    /// Draw the editor
    #[cfg(feature = "swash")]
    fn draw<F>(
        &self,
        font_system: &mut FontSystem,
        cache: &mut crate::SwashCache,
        _color: Color,
        mut f: F,
    ) where
        F: FnMut(i32, i32, u32, u32, Color),
    {
        let size = self.buffer().size();
        f(0, 0, size.0 as u32, size.1 as u32, self.background_color());
        self.editor
            .draw(font_system, cache, self.foreground_color(), f);
    }
}

impl<'a, 'b> BorrowedWithFontSystem<'b, SyntaxEditor<'a>> {
    /// Load text from a file, and also set syntax to the best option
    ///
    /// ## Errors
    ///
    /// Returns an [`io::Error`] if reading the file fails
    #[cfg(feature = "std")]
    pub fn load_text<P: AsRef<Path>>(&mut self, path: P, attrs: crate::Attrs) -> io::Result<()> {
        self.inner.load_text(self.font_system, path, attrs)
    }
}
