// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::{
    iced::{
        self,
        widget::{column, horizontal_space, pick_list, row},
        Alignment, Application, Color, Command, Length,
    },
    settings,
    theme::{self, Theme, ThemeType},
    widget::{button, text, toggler},
    Element,
};
use cosmic_text::{
    Align, Attrs, AttrsList, Buffer, Edit, FontSystem, LineHeight, SyntaxEditor, SyntaxSystem, Wrap,
};
use std::{env, fmt, fs, path::PathBuf, sync::Mutex};

use self::text_box::text_box;
mod text_box;

lazy_static::lazy_static! {
    static ref FONT_SYSTEM: Mutex<FontSystem> = Mutex::new(FontSystem::new());
    static ref SYNTAX_SYSTEM: SyntaxSystem = SyntaxSystem::new();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSize {
    Caption,
    Body,
    Title4,
    Title3,
    Title2,
    Title1,
}

impl FontSize {
    pub fn all() -> &'static [Self] {
        &[
            Self::Caption,
            Self::Body,
            Self::Title4,
            Self::Title3,
            Self::Title2,
            Self::Title1,
        ]
    }

    pub fn to_font_size(self) -> f32 {
        match self {
            Self::Caption => 10.0, // Caption
            Self::Body => 14.0,    // Body
            Self::Title4 => 20.0,  // Title 4
            Self::Title3 => 24.0,  // Title 3
            Self::Title2 => 28.0,  // Title 2
            Self::Title1 => 32.0,  // Title 1
        }
    }

    pub fn to_line_height(self) -> LineHeight {
        match self {
            Self::Caption => LineHeight::Absolute(14.0), // Caption
            Self::Body => LineHeight::Absolute(20.0),    // Body
            Self::Title4 => LineHeight::Absolute(28.0),  // Title 4
            Self::Title3 => LineHeight::Absolute(32.0),  // Title 3
            Self::Title2 => LineHeight::Absolute(36.0),  // Title 2
            Self::Title1 => LineHeight::Absolute(44.0),  // Title 1
        }
    }

    pub fn to_attrs(self) -> Attrs<'static> {
        Attrs::new()
            .font_size(self.to_font_size())
            .line_height(self.to_line_height())
    }
}

impl fmt::Display for FontSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Caption => write!(f, "Caption"),
            Self::Body => write!(f, "Body"),
            Self::Title4 => write!(f, "Title 4"),
            Self::Title3 => write!(f, "Title 3"),
            Self::Title2 => write!(f, "Title 2"),
            Self::Title1 => write!(f, "Title 1"),
        }
    }
}

static WRAP_MODE: &[Wrap] = &[Wrap::None, Wrap::Glyph, Wrap::Word];

fn main() -> cosmic::iced::Result {
    env_logger::init();

    let mut settings = settings();
    settings.window.min_size = Some((400, 100));
    Window::run(settings)
}

pub struct Window {
    theme: Theme,
    path_opt: Option<PathBuf>,
    attrs: Attrs<'static>,
    font_size: FontSize,
    #[cfg(not(feature = "vi"))]
    editor: Mutex<SyntaxEditor<'static>>,
    #[cfg(feature = "vi")]
    editor: Mutex<cosmic_text::ViEditor<'static>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Message {
    Open,
    Save,
    Bold(bool),
    Italic(bool),
    Monospaced(bool),
    FontSizeChanged(FontSize),
    WrapChanged(Wrap),
    AlignmentChanged(Align),
    ThemeChanged(&'static str),
}

impl Window {
    pub fn open(&mut self, path: PathBuf) {
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor.borrow_with(&mut font_system);
        match editor.load_text(&path, self.attrs) {
            Ok(()) => {
                log::info!("opened '{}'", path.display());
                self.path_opt = Some(path);
            }
            Err(err) => {
                log::error!("failed to open '{}': {}", path.display(), err);
                self.path_opt = None;
            }
        }
    }
}

impl Application for Window {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let attrs = FontSize::Body
            .to_attrs()
            .family(cosmic_text::Family::Monospace);

        let mut editor = SyntaxEditor::new(
            Buffer::new(&mut FONT_SYSTEM.lock().unwrap()),
            &SYNTAX_SYSTEM,
            "base16-eighties.dark",
        )
        .unwrap();

        #[cfg(feature = "vi")]
        let mut editor = cosmic_text::ViEditor::new(editor);

        update_attrs(&mut editor, attrs);

        let mut window = Window {
            theme: Theme::dark(),
            font_size: FontSize::Body,
            path_opt: None,
            attrs,
            editor: Mutex::new(editor),
        };
        if let Some(arg) = env::args().nth(1) {
            window.open(PathBuf::from(arg));
        }
        (window, Command::none())
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn title(&self) -> String {
        if let Some(path) = &self.path_opt {
            format!(
                "COSMIC Text - {} - {}",
                FONT_SYSTEM.lock().unwrap().locale(),
                path.display()
            )
        } else {
            format!("COSMIC Text - {}", FONT_SYSTEM.lock().unwrap().locale())
        }
    }

    fn update(&mut self, message: Message) -> iced::Command<Self::Message> {
        match message {
            Message::Open => {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.open(path);
                }
            }
            Message::Save => {
                if let Some(path) = &self.path_opt {
                    let editor = self.editor.lock().unwrap();
                    let mut text = String::new();
                    for line in editor.buffer().lines.iter() {
                        text.push_str(line.text());
                        text.push('\n');
                    }
                    match fs::write(path, text) {
                        Ok(()) => {
                            log::info!("saved '{}'", path.display());
                        }
                        Err(err) => {
                            log::error!("failed to save '{}': {}", path.display(), err);
                        }
                    }
                }
            }
            Message::Bold(bold) => {
                self.attrs = self.attrs.weight(if bold {
                    cosmic_text::Weight::BOLD
                } else {
                    cosmic_text::Weight::NORMAL
                });

                let mut editor = self.editor.lock().unwrap();
                update_attrs(&mut *editor, self.attrs);
            }
            Message::Italic(italic) => {
                self.attrs = self.attrs.style(if italic {
                    cosmic_text::Style::Italic
                } else {
                    cosmic_text::Style::Normal
                });

                let mut editor = self.editor.lock().unwrap();
                update_attrs(&mut *editor, self.attrs);
            }
            Message::Monospaced(monospaced) => {
                self.attrs = self.attrs.family(if monospaced {
                    cosmic_text::Family::Monospace
                } else {
                    cosmic_text::Family::SansSerif
                });

                let mut editor = self.editor.lock().unwrap();
                update_attrs(&mut *editor, self.attrs);
            }
            Message::FontSizeChanged(font_size) => {
                self.font_size = font_size;
                self.attrs = self
                    .attrs
                    .font_size(font_size.to_font_size())
                    .line_height(font_size.to_line_height());

                let mut editor = self.editor.lock().unwrap();
                update_attrs(&mut *editor, self.attrs);
            }
            Message::WrapChanged(wrap) => {
                let mut editor = self.editor.lock().unwrap();
                editor
                    .borrow_with(&mut FONT_SYSTEM.lock().unwrap())
                    .buffer_mut()
                    .set_wrap(wrap);
            }
            Message::AlignmentChanged(align) => {
                let mut editor = self.editor.lock().unwrap();
                update_alignment(&mut *editor, align);
            }
            Message::ThemeChanged(theme) => {
                self.theme = match theme {
                    "Dark" => Theme::dark(),
                    "Light" => Theme::light(),
                    _ => return Command::none(),
                };

                let Color { r, g, b, a } = self.theme.cosmic().on_bg_color().into();
                let as_u8 = |component: f32| (component * 255.0) as u8;
                self.attrs = self.attrs.color(cosmic_text::Color::rgba(
                    as_u8(r),
                    as_u8(g),
                    as_u8(b),
                    as_u8(a),
                ));

                let mut editor = self.editor.lock().unwrap();

                #[cfg(not(feature = "vi"))]
                // Update the syntax color theme
                match theme {
                    "Light" => editor.update_theme("base16-ocean.light"),
                    "Dark" | _ => editor.update_theme("base16-eighties.dark"),
                };

                update_attrs(&mut *editor, self.attrs);
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        static THEMES: &[&str] = &["Dark", "Light"];
        let theme_picker = pick_list(
            THEMES,
            Some(match self.theme.theme_type {
                ThemeType::Dark => THEMES[0],
                ThemeType::Light => THEMES[1],
                _ => unreachable!(),
            }),
            Message::ThemeChanged,
        );

        let font_size_picker = {
            pick_list(
                FontSize::all(),
                Some(self.font_size),
                Message::FontSizeChanged,
            )
        };

        let wrap_picker = {
            let editor = self.editor.lock().unwrap();
            pick_list(
                WRAP_MODE,
                Some(editor.buffer().wrap()),
                Message::WrapChanged,
            )
        };

        let content: Element<_> = column![
            row![
                button(theme::Button::Secondary)
                    .text("Open")
                    .on_press(Message::Open),
                button(theme::Button::Secondary)
                    .text("Save")
                    .on_press(Message::Save),
                horizontal_space(Length::Fill),
                text("Bold:"),
                toggler(
                    None,
                    self.attrs.weight == cosmic_text::Weight::BOLD,
                    Message::Bold
                ),
                text("Italic:"),
                toggler(
                    None,
                    self.attrs.style == cosmic_text::Style::Italic,
                    Message::Italic
                ),
                text("Monospaced:"),
                toggler(
                    None,
                    self.attrs.family == cosmic_text::Family::Monospace,
                    Message::Monospaced
                ),
                text("Theme:"),
                theme_picker,
                text("Font Size:"),
                font_size_picker,
            ]
            .align_items(Alignment::Center)
            .spacing(8),
            row![
                text("Wrap:"),
                wrap_picker,
                button(theme::Button::Text)
                    .icon(theme::Svg::Default, "format-justify-left", 20)
                    .on_press(Message::AlignmentChanged(Align::Left)),
                button(theme::Button::Text)
                    .icon(theme::Svg::Symbolic, "format-justify-center", 20)
                    .on_press(Message::AlignmentChanged(Align::Center)),
                button(theme::Button::Text)
                    .icon(theme::Svg::Symbolic, "format-justify-right", 20)
                    .on_press(Message::AlignmentChanged(Align::Right)),
                button(theme::Button::Text)
                    .icon(theme::Svg::SymbolicLink, "format-justify-fill", 20)
                    .on_press(Message::AlignmentChanged(Align::Justified)),
            ]
            .align_items(Alignment::Center)
            .spacing(8),
            text_box(&self.editor)
        ]
        .spacing(8)
        .padding(16)
        .into();

        // Uncomment to debug layout: content.explain(Color::WHITE)
        content
    }
}

fn update_attrs<T: Edit>(editor: &mut T, attrs: Attrs) {
    editor.buffer_mut().lines.iter_mut().for_each(|line| {
        line.set_attrs_list(AttrsList::new(attrs));
    });
}

fn update_alignment<T: Edit>(editor: &mut T, align: Align) {
    let current_line = editor.cursor().line;
    if let Some((start, end)) = editor.selection_bounds() {
        if let Some(lines) = editor.buffer_mut().lines.get_mut(start.line..=end.line) {
            for line in lines.iter_mut() {
                line.set_align(Some(align));
            }
        }
    } else if let Some(line) = editor.buffer_mut().lines.get_mut(current_line) {
        line.set_align(Some(align));
    }
}
