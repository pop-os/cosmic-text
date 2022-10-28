// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::{
    iced::{
        self,
        Alignment,
        Application,
        Command,
        Element,
        Length,
        Theme,
        widget::{
            column,
            horizontal_space,
            pick_list,
            row,
        },
    },
    settings,
    widget::{
        button,
        toggler,
    },
};
use cosmic_text::{
    Attrs,
    AttrsList,
    FontSystem,
    TextBuffer,
    TextMetrics,
};
use std::{
    env,
    fs,
    path::PathBuf,
    sync::Mutex,
};

use self::text_box::text_box;
mod text_box;

use self::text_new::text as text_new;
mod text_new;

lazy_static::lazy_static! {
    static ref FONT_SYSTEM: FontSystem<'static> = FontSystem::new();
}

static FONT_SIZES: &'static [TextMetrics] = &[
    TextMetrics::new(10, 14), // Caption
    TextMetrics::new(14, 20), // Body
    TextMetrics::new(20, 28), // Title 4
    TextMetrics::new(24, 32), // Title 3
    TextMetrics::new(28, 36), // Title 2
    TextMetrics::new(32, 44), // Title 1
];

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
    buffer: Mutex<TextBuffer<'static>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Message {
    Open,
    Save,
    Bold(bool),
    Italic(bool),
    Monospaced(bool),
    MetricsChanged(TextMetrics),
    ThemeChanged(&'static str),
}

impl Window {
    pub fn open(&mut self, path: PathBuf) {
        let mut buffer = self.buffer.lock().unwrap();
        match fs::read_to_string(&path) {
            Ok(text) => {
                log::info!("opened '{}'", path.display());
                buffer.set_text(&text, self.attrs);
                self.path_opt = Some(path);
            },
            Err(err) => {
                log::error!("failed to open '{}': {}", path.display(), err);
                buffer.set_text("", self.attrs);
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

        let buffer = TextBuffer::new(
            &FONT_SYSTEM,
            FONT_SIZES[1 /* Body */],
        );

        let mut window = Window {
            theme: Theme::Dark,
            path_opt: None,
            attrs,
            buffer: Mutex::new(buffer),
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
            format!("COSMIC Text - {} - {}", FONT_SYSTEM.locale, path.display())
        } else {
            format!("COSMIC Text - {}", FONT_SYSTEM.locale)
        }
    }

    fn update(&mut self, message: Message) -> iced::Command<Self::Message> {
        match message {
            Message::Open => {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.open(path);
                }
            },
            Message::Save => {
                if let Some(path) = &self.path_opt {
                    let buffer = self.buffer.lock().unwrap();
                    let mut text = String::new();
                    for line in buffer.lines.iter() {
                        text.push_str(line.text());
                        text.push('\n');
                    }
                    match fs::write(path, text) {
                        Ok(()) => {
                            log::info!("saved '{}'", path.display());
                        },
                        Err(err) => {
                            log::error!("failed to save '{}': {}", path.display(), err);
                        }
                    }
                }
            },
            Message::Bold(bold) => {
                self.attrs = self.attrs.weight(if bold {
                    cosmic_text::Weight::BOLD
                } else {
                    cosmic_text::Weight::NORMAL
                });

                let mut buffer = self.buffer.lock().unwrap();
                for line in buffer.lines.iter_mut() {
                    line.set_attrs_list(AttrsList::new(self.attrs));
                }
            },
            Message::Italic(italic) => {
                self.attrs = self.attrs.style(if italic {
                    cosmic_text::Style::Italic
                } else {
                    cosmic_text::Style::Normal
                });

                let mut buffer = self.buffer.lock().unwrap();
                for line in buffer.lines.iter_mut() {
                    line.set_attrs_list(AttrsList::new(self.attrs));
                }
            },
            Message::Monospaced(monospaced) => {
                self.attrs = self.attrs
                    .family(if monospaced {
                        cosmic_text::Family::Monospace
                    } else {
                        cosmic_text::Family::SansSerif
                    })
                    .monospaced(monospaced);

                let mut buffer = self.buffer.lock().unwrap();
                for line in buffer.lines.iter_mut() {
                    line.set_attrs_list(AttrsList::new(self.attrs));
                }
            },
            Message::MetricsChanged(metrics) => {
                let mut buffer = self.buffer.lock().unwrap();
                buffer.set_metrics(metrics);
            },
            Message::ThemeChanged(theme) => match theme {
                "Dark" => self.theme = Theme::Dark,
                "Light" => self.theme = Theme::Light,
                _ => (),
            },
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
            Message::ThemeChanged
        );

        let font_size_picker = {
            let buffer = self.buffer.lock().unwrap();
            pick_list(
                FONT_SIZES,
                Some(buffer.metrics()),
                Message::MetricsChanged
            )
        };

        let content: Element<_> = column![
            row![
                button!("Open").on_press(Message::Open),
                button!("Save").on_press(Message::Save),
                horizontal_space(Length::Fill),
                text_new("Bold:"),
                toggler(None, self.attrs.weight == cosmic_text::Weight::BOLD, Message::Bold),
                text_new("Italic:"),
                toggler(None, self.attrs.style == cosmic_text::Style::Italic, Message::Italic),
                text_new("Monospaced:"),
                toggler(None, self.attrs.monospaced, Message::Monospaced),
                text_new("Theme:"),
                theme_picker,
                text_new("Font Size:"),
                font_size_picker,
            ]
            .align_items(Alignment::Center)
            .spacing(8)
            ,
            text_box(&self.buffer)
        ]
        .spacing(8)
        .padding(16)
        .into();

        // Uncomment to debug layout: content.explain(Color::WHITE)
        content
    }
}
