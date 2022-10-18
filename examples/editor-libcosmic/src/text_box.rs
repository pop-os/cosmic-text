use cosmic::iced_native::{
    {Color, Element, Length, Point, Rectangle, Size, Shell},
    clipboard::Clipboard,
    event::{
        Event,
        Status,
    },
    keyboard::{Event as KeyEvent, KeyCode},
    layout::{self, Layout},
    renderer,
    widget::{self, Widget},
};
use cosmic_text::{
    FontLineIndex,
    TextAction,
    TextBuffer,
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub struct TextBox<'a> {
    buffer: Arc<Mutex<TextBuffer<'a>>>,
}

impl<'a> TextBox<'a> {
    pub fn new(buffer: Arc<Mutex<TextBuffer<'a>>>) -> Self {
        Self { buffer }
    }
}

pub fn text_box<'a>(buffer: Arc<Mutex<TextBuffer<'a>>>) -> TextBox<'a> {
    TextBox::new(buffer)
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for TextBox<'a>
where
    Renderer: renderer::Renderer,
{
    fn width(&self) -> Length {
        Length::Shrink
    }

    fn height(&self) -> Length {
        Length::Shrink
    }

    fn layout(
        &self,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        println!("{:?}", limits);
        let size = limits.max();
        {
            let mut buffer = self.buffer.lock().unwrap();

            buffer.set_size(size.width as i32, size.height as i32);
        }
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _state: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Renderer::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor_position: Point,
        _viewport: &Rectangle,
    ) {
        let buffer = self.buffer.lock().unwrap();

        let font_size = buffer.font_size();
        let line_height = buffer.line_height();

        let instant = Instant::now();

        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border_radius: 0.0,
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
            Color::from_rgb8(0x34, 0x34, 0x34),
        );

        let line_x = layout.bounds().x as i32;
        let mut line_y = layout.bounds().y as i32 + font_size;
        let mut start_line_opt = None;
        let mut end_line = FontLineIndex::new(0);
        for (line_i, line) in buffer
            .layout_lines()
            .iter()
            .skip(buffer.scroll as usize)
            .take(buffer.lines() as usize)
            .enumerate()
        {
            end_line = line.line_i;
            if start_line_opt == None {
                start_line_opt = Some(end_line);
            }

            if buffer.cursor.line == line_i + buffer.scroll as usize {
                if buffer.cursor.glyph >= line.glyphs.len() {
                    let x = match line.glyphs.last() {
                        Some(glyph) => glyph.x + glyph.w,
                        None => 0.0,
                    };

                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                Point::new(line_x as f32 + x, (line_y - font_size) as f32),
                                Size::new((font_size / 2) as f32, line_height as f32)
                            ),
                            border_radius: 0.0,
                            border_width: 0.0,
                            border_color: Color::TRANSPARENT,
                        },
                        Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.125),
                    );
                } else {
                    let glyph = &line.glyphs[buffer.cursor.glyph];
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                Point::new(line_x as f32 + glyph.x, (line_y - font_size) as f32),
                                Size::new(glyph.w, line_height as f32)
                            ),
                            border_radius: 0.0,
                            border_width: 0.0,
                            border_color: Color::TRANSPARENT,
                        },
                        Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.125),
                    );

                    let text_line = &buffer.text_lines()[line.line_i.get()];
                    log::info!(
                        "{}, {}: '{}' ('{}'): '{}'",
                        glyph.start,
                        glyph.end,
                        glyph.font.info.family,
                        glyph.font.info.post_script_name,
                        &text_line[glyph.start..glyph.end],
                    );
                }
            }

            line.draw(0xFFFFFF, |x, y, data| {
                let a = (data >> 24) as u8;
                if a > 0 {
                    let r = (data >> 16) as u8;
                    let g = (data >> 8) as u8;
                    let b = data as u8;
                    let bounds = Rectangle::new(
                        Point::new((line_x + x) as f32, (line_y + y) as f32),
                        Size::new(1.0, 1.0)
                    );
                    let color = Color::from_rgba8(r, g, b, a as f32 / 255.0);
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border_radius: 0.0,
                            border_width: 0.0,
                            border_color: Color::TRANSPARENT,
                        },
                        color
                    );
                }
            });

            line_y += line_height;
        }

        /*
        // Draw scrollbar
        {
            let start_line = start_line_opt.unwrap_or(end_line);
            let lines = buffer.text_lines().len();
            let start_y = (start_line.get() * window.height() as usize) / lines;
            let end_y = (end_line.get() * window.height() as usize) / lines;
            if end_y > start_y {
                window.rect(
                    window.width() as i32 - line_x as i32,
                    start_y as i32,
                    line_x as u32,
                    (end_y - start_y) as u32,
                    Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.25),
                );
            }
        }

        buffer.redraw = false;
        */

        let duration = instant.elapsed();
        log::debug!("redraw: {:?}", duration);
    }

    fn on_event(
        &mut self,
        _state: &mut widget::Tree,
        event: Event,
        _layout: Layout<'_>,
        _cursor_position: Point,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
    ) -> Status {
        let mut buffer = self.buffer.lock().unwrap();

        match event {
            Event::Keyboard(key_event) => match key_event {
                KeyEvent::KeyPressed { key_code, modifiers } => {
                    match key_code {
                        KeyCode::Left => {
                            buffer.action(TextAction::Left);
                            Status::Captured
                        },
                        KeyCode::Right => {
                            buffer.action(TextAction::Right);
                            Status::Captured
                        },
                        KeyCode::Up => {
                            buffer.action(TextAction::Up);
                            Status::Captured
                        },
                        KeyCode::Down => {
                            buffer.action(TextAction::Down);
                            Status::Captured
                        },
                        KeyCode::Backspace => {
                            buffer.action(TextAction::Backspace);
                            Status::Captured
                        },
                        KeyCode::Delete => {
                            buffer.action(TextAction::Delete);
                            Status::Captured
                        },
                        KeyCode::PageUp => {
                            buffer.action(TextAction::PageUp);
                            Status::Captured
                        },
                        KeyCode::PageDown => {
                            buffer.action(TextAction::PageDown);
                            Status::Captured
                        },
                        _ => Status::Ignored,
                    }
                },
                KeyEvent::CharacterReceived(character) => {
                    buffer.action(TextAction::Insert(character));
                    Status::Captured
                },
                _ => Status::Ignored,
            },
            _ => Status::Ignored,
        }
    }
}

impl<'a, Message, Renderer> From<TextBox<'a>> for Element<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn from(text_box: TextBox<'a>) -> Self {
        Self::new(text_box)
    }
}
