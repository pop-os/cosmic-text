use cosmic::{
    iced::widget::{
        container,
    },
    iced::{
        self,
        Application,
        Command,
        Element,
        Theme
    },
    settings,
};
use cosmic_text::{
    FontMatches,
    FontSystem,
    TextBuffer,
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
    buffer: Arc<Mutex<TextBuffer<'static>>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Message {}

impl Application for Window {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let font_sizes = [
            (10, 14), // Caption
            (14, 20), // Body
            (20, 28), // Title 4
            (24, 32), // Title 3
            (28, 36), // Title 2
            (32, 44), // Title 1
        ];
        let font_size_default = 1; // Body
        let mut font_size_i = font_size_default;

        let text = if let Some(arg) = env::args().nth(1) {
            fs::read_to_string(&arg).expect("failed to open file")
        } else {
            #[cfg(feature = "mono")]
            let default_text = include_str!("../../../sample/mono.txt");
            #[cfg(not(feature = "mono"))]
            let default_text = include_str!("../../../sample/proportional.txt");
            default_text.to_string()
        };

        let buffer = Arc::new(Mutex::new(TextBuffer::new(
            unsafe { FONT_MATCHES.as_ref().unwrap() },
            &text,
            font_sizes[font_size_i].0,
            font_sizes[font_size_i].1,
            0,
            0
        )));

        let window = Window {
            buffer,
        };
        (window, Command::none())
    }

    fn title(&self) -> String {
        let buffer = self.buffer.lock().unwrap();
        format!("COSMIC Text - iced - {}", buffer.font_matches().locale)
    }

    fn update(&mut self, message: Message) -> iced::Command<Self::Message> {
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        container(
            text_box(self.buffer.clone())
        ).padding(16).into()
    }
}
