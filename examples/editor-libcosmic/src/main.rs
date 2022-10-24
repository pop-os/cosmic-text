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
            text,
        },
    },
    settings,
    widget::{
        button,
    },
};
use cosmic_text::{
    FontMatches,
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

lazy_static::lazy_static! {
    static ref FONT_SYSTEM: FontSystem = FontSystem::new();
}

//TODO: find out how to do this!
static mut FONT_MATCHES: Option<FontMatches<'static>> = None;

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

    let font_matches: FontMatches<'static> = FONT_SYSTEM.matches(|info| -> bool {
        #[cfg(feature = "mono")]
        let monospaced = true;

        #[cfg(not(feature = "mono"))]
        let monospaced = false;

        let matched = {
            info.style == fontdb::Style::Normal &&
            info.weight == fontdb::Weight::NORMAL &&
            info.stretch == fontdb::Stretch::Normal &&
            (info.monospaced == monospaced || info.post_script_name.contains("Emoji"))
        };

        if matched {
            log::debug!(
                "{:?}: family '{}' postscript name '{}' style {:?} weight {:?} stretch {:?} monospaced {:?}",
                info.id,
                info.family,
                info.post_script_name,
                info.style,
                info.weight,
                info.stretch,
                info.monospaced
            );
        }

        matched
    }).unwrap();

    unsafe { FONT_MATCHES = Some(font_matches); }

    let mut settings = settings();
    settings.window.min_size = Some((400, 100));
    Window::run(settings)
}

pub struct Window {
    theme: Theme,
    path_opt: Option<PathBuf>,
    buffer: Mutex<TextBuffer<'static>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Message {
    Open,
    Save,
    MetricsChanged(TextMetrics),
    ThemeChanged(&'static str),
}

impl Window {
    pub fn open(&mut self, path: PathBuf) {
        let mut buffer = self.buffer.lock().unwrap();
        match fs::read_to_string(&path) {
            Ok(text) => {
                log::info!("opened '{}'", path.display());
                buffer.set_text(&text);
                self.path_opt = Some(path);
            },
            Err(err) => {
                log::error!("failed to open '{}': {}", path.display(), err);
                buffer.set_text("");
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
        let font_size_i = 1; // Body
        let buffer = TextBuffer::new(
            unsafe { FONT_MATCHES.as_ref().unwrap() },
            FONT_SIZES[font_size_i],
        );

        let mut window = Window {
            theme: Theme::Dark,
            path_opt: None,
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
                    for line in buffer.text_lines() {
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

        column![
            row![
                button!("Open").on_press(Message::Open),
                button!("Save").on_press(Message::Save),
                horizontal_space(Length::Fill),
                text("Theme:"),
                theme_picker,
                text("Font Size:"),
                font_size_picker,
            ]
            .align_items(Alignment::Center)
            .spacing(8)
            ,
            text_box(&self.buffer)
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
}
