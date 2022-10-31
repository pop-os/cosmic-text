// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::iced_native::{
    {Color, Element, Length, Point, Rectangle, Shell, Theme},
    clipboard::Clipboard,
    event::{Event, Status},
    image,
    keyboard::{Event as KeyEvent, KeyCode},
    layout::{self, Layout},
    mouse::{self, Button, Event as MouseEvent, ScrollDelta},
    renderer,
    widget::{self, tree, Widget},
};
use cosmic_text::{
    Action,
    Editor,
    SwashCache,
};
use std::{
    cmp,
    sync::Mutex,
    time::Instant,
};

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

pub struct TextBox<'a> {
    editor: &'a Mutex<Editor<'static>>,
}

impl<'a> TextBox<'a> {
    pub fn new(editor: &'a Mutex<Editor<'static>>) -> Self {
        Self {
            editor,
        }
    }
}

pub fn text_box<'a>(editor: &'a Mutex<Editor<'static>>) -> TextBox<'a> {
    TextBox::new(editor)
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for TextBox<'a>
where
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle>,
    Renderer::Theme: StyleSheet,
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

    fn layout(
        &self,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
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
        _viewport: &Rectangle,
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
                background_color
            );
        }

        let text_color = cosmic_text::Color::rgba(
            cmp::max(0, cmp::min(255, (appearance.text_color.r * 255.0) as i32)) as u8,
            cmp::max(0, cmp::min(255, (appearance.text_color.g * 255.0) as i32)) as u8,
            cmp::max(0, cmp::min(255, (appearance.text_color.b * 255.0) as i32)) as u8,
            cmp::max(0, cmp::min(255, (appearance.text_color.a * 255.0) as i32)) as u8,
        );

        let mut pixels_opt = state.pixels_opt.lock().unwrap();

        let mut editor = self.editor.lock().unwrap();

        let layout_w = layout.bounds().width as i32;
        let layout_h = layout.bounds().height as i32;
        editor.buffer.set_size(layout_w, layout_h);

        editor.shape_as_needed();
        //TODO: redraw on color change
        if editor.buffer.redraw || pixels_opt.is_none() {
            // Redraw buffer to image

            let instant = Instant::now();

            let mut pixels = vec![0; layout_w as usize * layout_h as usize * 4];

            editor.draw(&mut state.cache.lock().unwrap(), text_color, |start_x, start_y, w, h, color| {
                let alpha = (color.0 >> 24) & 0xFF;
                if alpha == 0 {
                    // Do not draw if alpha is zero
                    return;
                }

                for y in start_y..start_y + h as i32{
                    if y < 0 || y >= layout_h {
                        // Skip if y out of bounds
                        continue;
                    }

                    let offset_y = y as usize * layout_w as usize * 4;
                    for x in start_x..start_x + w as i32 {
                        if x < 0 || x >= layout_w {
                            // Skip if x out of bounds
                            continue;
                        }

                        let offset = offset_y + x as usize * 4;

                        let mut current =
                            pixels[offset] as u32 |
                            (pixels[offset + 1] as u32) << 8 |
                            (pixels[offset + 2] as u32) << 16 |
                            (pixels[offset + 3] as u32) << 24;

                        if alpha >= 255 || current == 0 {
                            // Alpha is 100% or current is null, replace with no blending
                            current = color.0;
                        } else {
                            // Alpha blend with current value
                            let n_alpha = 255 - alpha;
                            let rb = ((n_alpha * (current & 0x00FF00FF)) + (alpha * (color.0 & 0x00FF00FF))) >> 8;
                            let ag = (n_alpha * ((current & 0xFF00FF00) >> 8))
                                + (alpha * (0x01000000 | ((color.0 & 0x0000FF00) >> 8)));
                            current = (rb & 0x00FF00FF) | (ag & 0xFF00FF00);
                        }

                        pixels[offset] = current as u8;
                        pixels[offset + 1] = (current >> 8) as u8;
                        pixels[offset + 2] = (current >> 16) as u8;
                        pixels[offset + 3] = (current >> 24) as u8;
                    }
                }
            });

            *pixels_opt = Some((layout_w as u32, layout_h as u32, pixels));

            editor.buffer.redraw = false;

            let duration = instant.elapsed();
            log::debug!("redraw: {:?}", duration);
        }

        if let Some((w, h, pixels)) = &*pixels_opt {
            let handle = image::Handle::from_pixels(*w, *h, pixels.clone());
            image::Renderer::draw(renderer, handle, layout.bounds());
        }
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
            Event::Keyboard(KeyEvent::KeyPressed { key_code, modifiers }) => match key_code {
                KeyCode::Left => {
                    editor.action(Action::Left);
                    status = Status::Captured;
                },
                KeyCode::Right => {
                    editor.action(Action::Right);
                    status = Status::Captured;
                },
                KeyCode::Up => {
                    editor.action(Action::Up);
                    status = Status::Captured;
                },
                KeyCode::Down => {
                    editor.action(Action::Down);
                    status = Status::Captured;
                },
                KeyCode::Home => {
                    editor.action(Action::Home);
                    status = Status::Captured;
                },
                KeyCode::End => {
                    editor.action(Action::End);
                    status = Status::Captured;
                },
                KeyCode::PageUp => {
                    editor.action(Action::PageUp);
                    status = Status::Captured;
                },
                KeyCode::PageDown => {
                    editor.action(Action::PageDown);
                    status = Status::Captured;
                },
                KeyCode::Enter => {
                    editor.action(Action::Enter);
                    status = Status::Captured;
                },
                KeyCode::Backspace => {
                    editor.action(Action::Backspace);
                    status = Status::Captured;
                },
                KeyCode::Delete => {
                    editor.action(Action::Delete);
                    status = Status::Captured;
                },
                _ => ()
            },
            Event::Keyboard(KeyEvent::CharacterReceived(character)) => {
                editor.action(Action::Insert(character));
                status = Status::Captured;
            },
            Event::Mouse(MouseEvent::ButtonPressed(Button::Left)) => {
                if layout.bounds().contains(cursor_position) {
                    editor.action(Action::Click {
                        x: (cursor_position.x - layout.bounds().x) as i32,
                        y: (cursor_position.y - layout.bounds().y) as i32,
                    });
                    state.is_dragging = true;
                    status = Status::Captured;
                }
            },
            Event::Mouse(MouseEvent::ButtonReleased(Button::Left)) => {
                state.is_dragging = false;
                status = Status::Captured;
            },
            Event::Mouse(MouseEvent::CursorMoved { .. }) => {
                if state.is_dragging {
                    editor.action(Action::Drag {
                        x: (cursor_position.x - layout.bounds().x) as i32,
                        y: (cursor_position.y - layout.bounds().y) as i32,
                    });
                    status = Status::Captured;
                }
            },
            Event::Mouse(MouseEvent::WheelScrolled { delta }) => match delta {
                ScrollDelta::Lines { x, y } => {
                    editor.action(Action::Scroll {
                        lines: (-y * 6.0) as i32,
                    });
                    status = Status::Captured;
                },
                _ => (),
            },
            _ => ()
        }

        status
    }
}

impl<'a, Message, Renderer> From<TextBox<'a>> for Element<'a, Message, Renderer>
where
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle>,
    Renderer::Theme: StyleSheet,
{
    fn from(text_box: TextBox<'a>) -> Self {
        Self::new(text_box)
    }
}

pub struct State {
    is_dragging: bool,
    cache: Mutex<SwashCache<'static>>,
    pixels_opt: Mutex<Option<(u32, u32, Vec<u8>)>>,
}

impl State {
    /// Creates a new [`State`].
    pub fn new() -> State {
        State {
            is_dragging: false,
            cache: Mutex::new(SwashCache::new(&crate::FONT_SYSTEM)),
            pixels_opt: Mutex::new(None),
        }
    }
}
