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
    Attrs, AttrsList, Buffer, Edit, FontSystem, Metrics, SyntaxEditor, SyntaxSystem, Wrap,
};
use std::{env, fs, path::PathBuf, sync::Mutex};

use self::text::text;
mod text;

use self::text_box::text_box;
mod text_box;

lazy_static::lazy_static! {
    static ref FONT_SYSTEM: FontSystem = FontSystem::new();
    static ref SYNTAX_SYSTEM: SyntaxSystem = SyntaxSystem::new();
}

static FONT_SIZES: &'static [Metrics] = &[
    Metrics::new(10, 14), // Caption
    Metrics::new(14, 20), // Body
    Metrics::new(20, 28), // Title 4
    Metrics::new(24, 32), // Title 3
    Metrics::new(28, 36), // Title 2
    Metrics::new(32, 44), // Title 1
];

static WRAP_MODE: &'static [Wrap] = &[Wrap::None, Wrap::Glyph, Wrap::Word];

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
    MetricsChanged(Metrics),
    WrapChanged(Wrap),
    ThemeChanged(&'static str),
}

impl Window {
    pub fn open(&mut self, path: PathBuf) {
        let mut editor = self.editor.lock().unwrap();
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

        let mut editor = SyntaxEditor::new(
            Buffer::new(&FONT_SYSTEM, FONT_SIZES[1 /* Body */]),
            &SYNTAX_SYSTEM,
            "base16-eighties.dark",
        )
        .unwrap();

        #[cfg(feature = "vi")]
        let mut editor = cosmic_text::ViEditor::new(editor);

        update_attrs(&mut editor, attrs);

        let mut window = Window {
            theme: Theme::Dark,
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
        self.theme
    }

    fn title(&self) -> String {
        if let Some(path) = &self.path_opt {
            format!(
                "COSMIC Text - {} - {}",
                FONT_SYSTEM.locale(),
                path.display()
            )
        } else {
            format!("COSMIC Text - {}", FONT_SYSTEM.locale())
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
                self.attrs = self
                    .attrs
                    .family(if monospaced {
                        cosmic_text::Family::Monospace
                    } else {
                        cosmic_text::Family::SansSerif
                    })
                    .monospaced(monospaced);

                let mut editor = self.editor.lock().unwrap();
                update_attrs(&mut *editor, self.attrs);
            }
            Message::MetricsChanged(metrics) => {
                let mut editor = self.editor.lock().unwrap();
                editor.buffer_mut().set_metrics(metrics);
            }
            Message::WrapChanged(wrap) => {
                let mut editor = self.editor.lock().unwrap();
                editor.buffer_mut().set_wrap(wrap);
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

                let mut editor = self.editor.lock().unwrap();
                update_attrs(&mut *editor, self.attrs);
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        static THEMES: &'static [&'static str] = &["Dark", "Light"];
        let theme_picker = pick_list(
            THEMES,
            Some(match self.theme {
                Theme::Dark => THEMES[0],
                Theme::Light => THEMES[1],
            }),
            Message::ThemeChanged,
        );

        let font_size_picker = {
            let editor = self.editor.lock().unwrap();
            pick_list(
                FONT_SIZES,
                Some(editor.buffer().metrics()),
                Message::MetricsChanged,
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
                toggler(None, self.attrs.monospaced, Message::Monospaced),
                text("Theme:"),
                theme_picker,
                text("Font Size:"),
                font_size_picker,
                text("Wrap:"),
                wrap_picker,
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

fn update_attrs<'a, T: Edit<'a>>(editor: &mut T, attrs: Attrs<'a>) {
    editor.buffer_mut().lines.iter_mut().for_each(|line| {
        line.set_attrs_list(AttrsList::new(attrs));
    });
}
