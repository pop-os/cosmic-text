// SPDX-License-Identifier: MIT OR Apache-2.0

use super::text;
use cosmic::{
    iced_native::{
        clipboard::Clipboard,
        event::{Event, Status},
        image,
        keyboard::{Event as KeyEvent, KeyCode},
        layout::{self, Layout},
        mouse::{self, Button, Event as MouseEvent, ScrollDelta},
        renderer,
        widget::{self, tree, Widget},
        Padding, {Color, Element, Length, Point, Rectangle, Shell, Size},
    },
    theme::Theme,
};
use cosmic_text::{Action, Edit, SwashCache};
use std::{cmp, sync::Mutex, time::Instant};

pub struct Appearance {
    background_color: Option<Color>,
    text_color: Color,
}

pub trait StyleSheet {
    fn appearance(&self) -> Appearance;
}

impl StyleSheet for Theme {
    fn appearance(&self) -> Appearance {
        match self {
            Theme::Dark => Appearance {
                background_color: Some(Color::from_rgb8(0x34, 0x34, 0x34)),
                text_color: Color::from_rgb8(0xFF, 0xFF, 0xFF),
            },
            Theme::Light => Appearance {
                background_color: Some(Color::from_rgb8(0xFC, 0xFC, 0xFC)),
                text_color: Color::from_rgb8(0x00, 0x00, 0x00),
            },
        }
    }
}

pub struct TextBox<'a, Editor> {
    editor: &'a Mutex<Editor>,
    padding: Padding,
}

impl<'a, Editor> TextBox<'a, Editor> {
    pub fn new(editor: &'a Mutex<Editor>) -> Self {
        Self {
            editor,
            padding: Padding::new(0),
        }
    }

    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }
}

pub fn text_box<'a, Editor>(editor: &'a Mutex<Editor>) -> TextBox<'a, Editor> {
    TextBox::new(editor)
}

impl<'a, 'editor, Editor, Message, Renderer> Widget<Message, Renderer> for TextBox<'a, Editor>
where
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle>,
    Renderer::Theme: StyleSheet,
    Editor: Edit<'editor>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn width(&self) -> Length {
        Length::Fill
    }

    fn height(&self) -> Length {
        Length::Fill
    }

    fn layout(&self, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let limits = limits.width(Length::Fill).height(Length::Fill);

        //TODO: allow lazy shape
        let mut editor = self.editor.lock().unwrap();
        editor.buffer_mut().shape_until(i32::max_value());

        let mut layout_lines = 0;
        for line in editor.buffer().lines.iter() {
            match line.layout_opt() {
                Some(layout) => layout_lines += layout.len(),
                None => (),
            }
        }

        let height = layout_lines as f32 * editor.buffer().metrics().line_height as f32;
        let size = Size::new(limits.max().width, height);
        log::info!("size {:?}", size);

        layout::Node::new(limits.resolve(size))
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor_position: Point,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if layout.bounds().contains(cursor_position) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::Idle
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Renderer::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor_position: Point,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let appearance = theme.appearance();

        if let Some(background_color) = appearance.background_color {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border_radius: 0.0,
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
                background_color,
            );
        }

        let text_color = cosmic_text::Color::rgba(
            cmp::max(0, cmp::min(255, (appearance.text_color.r * 255.0) as i32)) as u8,
            cmp::max(0, cmp::min(255, (appearance.text_color.g * 255.0) as i32)) as u8,
            cmp::max(0, cmp::min(255, (appearance.text_color.b * 255.0) as i32)) as u8,
            cmp::max(0, cmp::min(255, (appearance.text_color.a * 255.0) as i32)) as u8,
        );

        let mut editor = self.editor.lock().unwrap();

        let view_w = cmp::min(viewport.width as i32, layout.bounds().width as i32)
            - self.padding.horizontal() as i32;
        let view_h = cmp::min(viewport.height as i32, layout.bounds().height as i32)
            - self.padding.vertical() as i32;
        editor.buffer_mut().set_size(view_w, view_h);

        editor.shape_as_needed();

        let instant = Instant::now();

        let mut pixels = vec![0; view_w as usize * view_h as usize * 4];

        editor.draw(
            &mut state.cache.lock().unwrap(),
            text_color,
            |x, y, w, h, color| {
                if w <= 0 || h <= 0 {
                    // Do not draw invalid sized rectangles
                    return;
                }

                if w > 1 || h > 1 {
                    // Draw rectangles with optimized quad renderer
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                layout.position()
                                    + [x as f32, y as f32].into()
                                    + [self.padding.left as f32, self.padding.top as f32].into(),
                                Size::new(w as f32, h as f32),
                            ),
                            border_radius: 0.0,
                            border_width: 0.0,
                            border_color: Color::TRANSPARENT,
                        },
                        Color::from_rgba8(
                            color.r(),
                            color.g(),
                            color.b(),
                            (color.a() as f32) / 255.0,
                        ),
                    );
                } else {
                    text::draw_pixel(&mut pixels, view_w, view_h, x, y, color);
                }
            },
        );

        let handle = image::Handle::from_pixels(view_w as u32, view_h as u32, pixels);
        image::Renderer::draw(
            renderer,
            handle,
            Rectangle::new(
                layout.position() + [self.padding.left as f32, self.padding.top as f32].into(),
                Size::new(view_w as f32, view_h as f32),
            ),
        );

        let duration = instant.elapsed();
        log::debug!("redraw {}, {}: {:?}", view_w, view_h, duration);
    }

    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor_position: Point,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();
        let mut editor = self.editor.lock().unwrap();

        let mut status = Status::Ignored;
        match event {
            Event::Keyboard(KeyEvent::KeyPressed {
                key_code,
                modifiers,
            }) => match key_code {
                KeyCode::Left => {
                    editor.action(Action::Left);
                    status = Status::Captured;
                }
                KeyCode::Right => {
                    editor.action(Action::Right);
                    status = Status::Captured;
                }
                KeyCode::Up => {
                    editor.action(Action::Up);
                    status = Status::Captured;
                }
                KeyCode::Down => {
                    editor.action(Action::Down);
                    status = Status::Captured;
                }
                KeyCode::Home => {
                    editor.action(Action::Home);
                    status = Status::Captured;
                }
                KeyCode::End => {
                    editor.action(Action::End);
                    status = Status::Captured;
                }
                KeyCode::PageUp => {
                    editor.action(Action::PageUp);
                    status = Status::Captured;
                }
                KeyCode::PageDown => {
                    editor.action(Action::PageDown);
                    status = Status::Captured;
                }
                KeyCode::Escape => {
                    editor.action(Action::Escape);
                    status = Status::Captured;
                }
                KeyCode::Enter => {
                    editor.action(Action::Enter);
                    status = Status::Captured;
                }
                KeyCode::Backspace => {
                    editor.action(Action::Backspace);
                    status = Status::Captured;
                }
                KeyCode::Delete => {
                    editor.action(Action::Delete);
                    status = Status::Captured;
                }
                _ => (),
            },
            Event::Keyboard(KeyEvent::CharacterReceived(character)) => {
                editor.action(Action::Insert(character));
                status = Status::Captured;
            }
            Event::Mouse(MouseEvent::ButtonPressed(Button::Left)) => {
                if layout.bounds().contains(cursor_position) {
                    editor.action(Action::Click {
                        x: (cursor_position.x - layout.bounds().x) as i32
                            - self.padding.left as i32,
                        y: (cursor_position.y - layout.bounds().y) as i32 - self.padding.top as i32,
                    });
                    state.is_dragging = true;
                    status = Status::Captured;
                }
            }
            Event::Mouse(MouseEvent::ButtonReleased(Button::Left)) => {
                state.is_dragging = false;
                status = Status::Captured;
            }
            Event::Mouse(MouseEvent::CursorMoved { .. }) => {
                if state.is_dragging {
                    editor.action(Action::Drag {
                        x: (cursor_position.x - layout.bounds().x) as i32
                            - self.padding.left as i32,
                        y: (cursor_position.y - layout.bounds().y) as i32 - self.padding.top as i32,
                    });
                    status = Status::Captured;
                }
            }
            Event::Mouse(MouseEvent::WheelScrolled { delta }) => match delta {
                ScrollDelta::Lines { x, y } => {
                    editor.action(Action::Scroll {
                        lines: (-y * 6.0) as i32,
                    });
                    status = Status::Captured;
                }
                _ => (),
            },
            _ => (),
        }

        status
    }
}

impl<'a, 'editor, Editor, Message, Renderer> From<TextBox<'a, Editor>>
    for Element<'a, Message, Renderer>
where
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle>,
    Renderer::Theme: StyleSheet,
    Editor: Edit<'editor>,
{
    fn from(text_box: TextBox<'a, Editor>) -> Self {
        Self::new(text_box)
    }
}

pub struct State {
    is_dragging: bool,
    cache: Mutex<SwashCache<'static>>,
}

impl State {
    /// Creates a new [`State`].
    pub fn new() -> State {
        State {
            is_dragging: false,
            cache: Mutex::new(SwashCache::new(&crate::FONT_SYSTEM)),
        }
    }
}
