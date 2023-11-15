// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::{
    iced_core::{event::Status, widget::tree, *},
    iced_runtime::keyboard::KeyCode,
    theme::{Theme, ThemeType},
};
use cosmic_text::{Action, Edit, SwashCache};
use std::{cmp, sync::Mutex, time::Instant};

use crate::FONT_SYSTEM;

pub struct Appearance {
    background_color: Option<Color>,
    text_color: Color,
}

pub trait StyleSheet {
    fn appearance(&self) -> Appearance;
}

impl StyleSheet for Theme {
    fn appearance(&self) -> Appearance {
        match self.theme_type {
            ThemeType::Dark | ThemeType::HighContrastDark | ThemeType::Custom(_) => Appearance {
                background_color: Some(Color::from_rgb8(0x34, 0x34, 0x34)),
                text_color: Color::from_rgb8(0xFF, 0xFF, 0xFF),
            },
            ThemeType::Light | ThemeType::HighContrastLight => Appearance {
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
            padding: Padding::new(0.),
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

fn draw_pixel(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    color: cosmic_text::Color,
) {
    let alpha = (color.0 >> 24) & 0xFF;
    if alpha == 0 {
        // Do not draw if alpha is zero
        return;
    }

    if y < 0 || y >= height {
        // Skip if y out of bounds
        return;
    }

    if x < 0 || x >= width {
        // Skip if x out of bounds
        return;
    }

    let offset = (y as usize * width as usize + x as usize) * 4;

    let mut current = buffer[offset + 2] as u32
        | (buffer[offset + 1] as u32) << 8
        | (buffer[offset + 0] as u32) << 16
        | (buffer[offset + 3] as u32) << 24;

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

    buffer[offset + 2] = current as u8;
    buffer[offset + 1] = (current >> 8) as u8;
    buffer[offset + 0] = (current >> 16) as u8;
    buffer[offset + 3] = (current >> 24) as u8;
}

impl<'a, 'editor, Editor, Message, Renderer> Widget<Message, Renderer> for TextBox<'a, Editor>
where
    Renderer: cosmic::iced_core::Renderer + image::Renderer<Handle = image::Handle>,
    Renderer::Theme: StyleSheet,
    Editor: Edit,
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
        editor
            .borrow_with(&mut FONT_SYSTEM.lock().unwrap())
            .buffer_mut()
            .shape_until(i32::max_value());

        let mut height = 0.0;
        for line in editor.buffer().lines.iter() {
            let line_height = line
                .attrs_list()
                .spans()
                .iter()
                .map(|(_, attrs)| attrs.line_height.height(attrs.font_size))
                .max_by(f32::total_cmp)
                .unwrap_or(0.0);
            match line.layout_opt() {
                Some(layout) => height += layout.len() as f32 * line_height,
                None => (),
            }
        }

        let size = Size::new(limits.max().width, height);
        log::info!("size {:?}", size);

        layout::Node::new(limits.resolve(size))
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor_position: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor_position.is_over(layout.bounds()) {
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
        _cursor_position: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let instant = Instant::now();

        let state = tree.state.downcast_ref::<State>();

        let appearance = theme.appearance();

        if let Some(background_color) = appearance.background_color {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border_radius: 0.0.into(),
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

        const SCALE_FACTOR: f64 = 1.;

        let image_w = (view_w as f64 * SCALE_FACTOR) as i32;
        let image_h = (view_h as f64 * SCALE_FACTOR) as i32;

        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor.borrow_with(&mut font_system);

        // Scale metrics
        // let metrics = editor.buffer().metrics();
        // editor
        //     .buffer_mut()
        //     .set_metrics(metrics.scale(SCALE_FACTOR as f32));

        // Set size
        editor.buffer_mut().set_size(image_w as f32, image_h as f32);

        // Shape and layout
        editor.shape_as_needed();

        // Draw to pixel buffer
        let mut pixels = vec![0; image_w as usize * image_h as usize * 4];
        editor.draw(
            &mut state.cache.lock().unwrap(),
            text_color,
            |x, y, w, h, color| {
                //TODO: improve performance
                for row in 0..h as i32 {
                    for col in 0..w as i32 {
                        draw_pixel(&mut pixels, image_w, image_h, x + col, y + row, color);
                    }
                }
            },
        );

        // Restore original metrics
        // editor.buffer_mut().set_metrics(metrics);

        let handle = image::Handle::from_pixels(image_w as u32, image_h as u32, pixels);
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
        cursor_position: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor.borrow_with(&mut font_system);

        let mut status = Status::Ignored;
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key_code, .. }) => match key_code {
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
            Event::Keyboard(keyboard::Event::CharacterReceived(character)) => {
                editor.action(Action::Insert(character));
                status = Status::Captured;
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position_in) = cursor_position.position_in(layout.bounds()) {
                    editor.action(Action::Click {
                        x: position_in.x as i32 - self.padding.left as i32,
                        y: position_in.y as i32 - self.padding.top as i32,
                    });
                    state.is_dragging = true;
                    status = Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.is_dragging = false;
                status = Status::Captured;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.is_dragging {
                    editor.action(Action::Drag {
                        x: (position.x - layout.bounds().x) as i32 - self.padding.left as i32,
                        y: (position.y - layout.bounds().y) as i32 - self.padding.top as i32,
                    });
                    status = Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } => {
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
    Editor: Edit,
{
    fn from(text_box: TextBox<'a, Editor>) -> Self {
        Self::new(text_box)
    }
}

pub struct State {
    is_dragging: bool,
    cache: Mutex<SwashCache>,
}

impl State {
    /// Creates a new [`State`].
    pub fn new() -> State {
        State {
            is_dragging: false,
            cache: Mutex::new(SwashCache::new()),
        }
    }
}
