// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::{
    iced::{
        self,
        widget::{column, horizontal_space, pick_list, row},
        Alignment, Application, Color, Command, Length,
    },
    settings,
    theme::{self, Theme},
    widget::{button, toggler},
    Element,
};
use cosmic_text::{
    Align, Attrs, AttrsList, Buffer, BufferData, Edit, FontSystem, Metrics, SyntaxEditor,
    SyntaxSystem, Wrap,
};
use std::{env, fmt, fs, path::PathBuf, sync::Mutex};

use self::text::text;
mod text;

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

    pub fn to_metrics(self) -> Metrics {
        match self {
            Self::Caption => Metrics::new(10.0, 14.0), // Caption
            Self::Body => Metrics::new(14.0, 20.0),    // Body
            Self::Title4 => Metrics::new(20.0, 28.0),  // Title 4
            Self::Title3 => Metrics::new(24.0, 32.0),  // Title 3
            Self::Title2 => Metrics::new(28.0, 36.0),  // Title 2
            Self::Title1 => Metrics::new(32.0, 44.0),  // Title 1
        }
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
    buffer_data: Mutex<BufferData>,
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

#[cfg(not(feature = "vi"))]
type Editor<'a> = SyntaxEditor<'a>;

#[cfg(feature = "vi")]
type Editor<'a> = cosmic_text::ViEditor<'a>;

#[cfg(not(feature = "vi"))]
fn editor<'a>(font_system: &'a mut FontSystem, buffer_data: &'a mut BufferData) -> Editor<'a> {
    let editor = SyntaxEditor::new(
        Buffer::new(font_system, buffer_data),
        &SYNTAX_SYSTEM,
        "base16-eighties.dark",
    )
    .unwrap();

    #[cfg(feature = "vi")]
    let editor = cosmic_text::ViEditor::new(editor);

    editor
}

impl Window {
    pub fn open(&mut self, path: PathBuf) {
        let mut buffer_data = self.buffer_data.lock().unwrap();
        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor(&mut font_system, &mut buffer_data);
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
        let attrs = cosmic_text::Attrs::new()
            .monospaced(true)
            .family(cosmic_text::Family::Monospace);

        let mut font_system = FONT_SYSTEM.lock().unwrap();

        let mut buffer_data = BufferData::new(&mut font_system, FontSize::Body.to_metrics());

        let mut editor = SyntaxEditor::new(
            Buffer::new(&mut font_system, &mut buffer_data),
            &SYNTAX_SYSTEM,
            "base16-eighties.dark",
        )
        .unwrap();

        #[cfg(feature = "vi")]
        let mut editor = cosmic_text::ViEditor::new(editor);

        update_attrs(&mut editor, attrs);

        let mut window = Window {
            theme: Theme::Dark,
            font_size: FontSize::Body,
            path_opt: None,
            attrs,
            buffer_data: Mutex::new(buffer_data),
        };
        if let Some(arg) = env::args().nth(1) {
            window.open(PathBuf::from(arg));
        }
        (window, Command::none())
    }

    fn theme(&self) -> Theme {
        self.theme
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
                    let mut buffer_data = self.buffer_data.lock().unwrap();
                    let mut font_system = FONT_SYSTEM.lock().unwrap();
                    let editor = editor(&mut font_system, &mut buffer_data);
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

                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                update_attrs(&mut editor, self.attrs);
            }
            Message::Italic(italic) => {
                self.attrs = self.attrs.style(if italic {
                    cosmic_text::Style::Italic
                } else {
                    cosmic_text::Style::Normal
                });

                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                update_attrs(&mut editor, self.attrs);
            }
            Message::Monospaced(monospaced) => {
                self.attrs = self
                    .attrs
                    .family(if monospaced {
                        cosmic_text::Family::Monospace
                    } else {
                        cosmic_text::Family::SansSerif
                    })
                    .monospaced(monospaced);

                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                update_attrs(&mut editor, self.attrs);
            }
            Message::FontSizeChanged(font_size) => {
                self.font_size = font_size;
                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                editor.buffer_mut().set_metrics(font_size.to_metrics());
            }
            Message::WrapChanged(wrap) => {
                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                editor.buffer_mut().set_wrap(wrap);
            }
            Message::AlignmentChanged(align) => {
                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                update_alignment(&mut editor, align);
            }
            Message::ThemeChanged(theme) => {
                self.theme = match theme {
                    "Dark" => Theme::Dark,
                    "Light" => Theme::Light,
                    _ => return Command::none(),
                };

                let Color { r, g, b, a } = self.theme.palette().text;
                let as_u8 = |component: f32| (component * 255.0) as u8;
                self.attrs = self.attrs.color(cosmic_text::Color::rgba(
                    as_u8(r),
                    as_u8(g),
                    as_u8(b),
                    as_u8(a),
                ));

                let mut buffer_data = self.buffer_data.lock().unwrap();
                let mut font_system = FONT_SYSTEM.lock().unwrap();
                let mut editor = editor(&mut font_system, &mut buffer_data);
                update_attrs(&mut editor, self.attrs);
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        static THEMES: &[&str] = &["Dark", "Light"];
        let theme_picker = pick_list(
            THEMES,
            Some(match self.theme {
                Theme::Dark => THEMES[0],
                Theme::Light => THEMES[1],
            }),
            Message::ThemeChanged,
        );

        let font_size_picker = {
            let mut buffer_data = self.buffer_data.lock().unwrap();
            let mut font_system = FONT_SYSTEM.lock().unwrap();
            let editor = editor(&mut font_system, &mut buffer_data);
            pick_list(
                FontSize::all(),
                Some(self.font_size),
                Message::FontSizeChanged,
            )
        };

        let wrap_picker = {
            let mut buffer_data = self.buffer_data.lock().unwrap();
            let mut font_system = FONT_SYSTEM.lock().unwrap();
            let editor = editor(&mut font_system, &mut buffer_data);
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
                toggler(None, self.attrs.monospaced, Message::Monospaced),
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
            text_box(&self.buffer_data)
        ]
        .spacing(8)
        .padding(16)
        .into();

        // Uncomment to debug layout: content.explain(Color::WHITE)
        content
    }
}

fn update_attrs<'a, T: Edit<'a>>(editor: &mut T, attrs: Attrs<'a>) {
    editor.buffer_mut().lines.iter_mut().for_each(|line| {
        line.set_attrs_list(AttrsList::new(attrs));
    });
}

fn update_alignment<'a, T: Edit<'a>>(editor: &mut T, align: Align) {
    let current_line = editor.cursor().line;
    if let Some(select) = editor.select_opt() {
        let (start, end) = match select.line.cmp(&current_line) {
            std::cmp::Ordering::Greater => (current_line, select.line),
            std::cmp::Ordering::Less => (select.line, current_line),
            std::cmp::Ordering::Equal => (current_line, current_line),
        };
        if let Some(lines) = editor.buffer_mut().lines.get_mut(start..=end) {
            for line in lines.iter_mut() {
                line.set_align(Some(align));
            }
        }
    } else if let Some(line) = editor.buffer_mut().lines.get_mut(current_line) {
        line.set_align(Some(align));
    }
}
