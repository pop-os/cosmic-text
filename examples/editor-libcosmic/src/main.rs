use cosmic::{
    iced::{
        self,
        Alignment,
        Application,
        Command,
        Element,
        Theme,
        widget::{
            column,
            pick_list,
            row,
            text,
        },
    },
    settings,
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
    sync::{Arc, Mutex},
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
    buffer: Arc<Mutex<TextBuffer<'static>>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Message {
    MetricsChanged(TextMetrics),
    ThemeChanged(&'static str),
}

impl Application for Window {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let text = if let Some(arg) = env::args().nth(1) {
            fs::read_to_string(&arg).expect("failed to open file")
        } else {
            #[cfg(feature = "mono")]
            let default_text = include_str!("../../../sample/mono.txt");
            #[cfg(not(feature = "mono"))]
            let default_text = include_str!("../../../sample/proportional.txt");
            default_text.to_string()
        };

        let font_size_i = 1; // Body
        let buffer = Arc::new(Mutex::new(TextBuffer::new(
            unsafe { FONT_MATCHES.as_ref().unwrap() },
            &text,
            FONT_SIZES[font_size_i],
        )));

        let window = Window {
            theme: Theme::Dark,
            buffer,
        };
        (window, Command::none())
    }

    fn theme(&self) -> Theme {
        self.theme
    }

    fn title(&self) -> String {
        format!("COSMIC Text - iced - {}", FONT_SYSTEM.locale)
    }

    fn update(&mut self, message: Message) -> iced::Command<Self::Message> {
        match message {
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
                text("Theme:"),
                theme_picker,
                text("Font Size:"),
                font_size_picker,
            ]
            .align_items(Alignment::Center)
            .spacing(8)
            ,
            text_box(self.buffer.clone())
        ]
        .spacing(8)
        .padding(16)
        .into()
    }
}
