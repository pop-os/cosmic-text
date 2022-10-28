// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::iced_native::{
    {Color, Element, Length, Point, Rectangle, Size, Theme},
    layout::{self, Layout},
    renderer,
    widget::{self, tree, Widget},
};
use cosmic_text::{
    Attrs,
    AttrsList,
    SwashCache,
    TextBufferLine,
    TextMetrics,
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
                background_color: None,
                text_color: Color::from_rgb8(0xFF, 0xFF, 0xFF),
            },
            Theme::Light => Appearance {
                background_color: None,
                text_color: Color::from_rgb8(0x00, 0x00, 0x00),
            },
        }
    }
}

pub struct Text {
    line: TextBufferLine<'static>,
    metrics: TextMetrics,
}

impl Text {
    pub fn new(string: &str) -> Self {
        let instant = Instant::now();

        //TODO: make it possible to set attrs
        let mut line = TextBufferLine::new(
            string,
            AttrsList::new(Attrs::new())
        );

        //TODO: do we have to immediately chape?
        line.shape(&crate::FONT_SYSTEM);

        let text = Self {
            line,
            metrics: TextMetrics::new(14, 20),
        };

        log::debug!("Text::new in {:?}", instant.elapsed());

        text
    }
}

pub fn text(string: &str) -> Text {
    Text::new(string)
}

impl<Message, Renderer> Widget<Message, Renderer> for Text
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

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
        let instant = Instant::now();

        let shape = self.line.shape_opt().as_ref().unwrap();

        //TODO: can we cache this?
        let mut layout_lines = Vec::new();
        shape.layout(
            self.metrics.font_size,
            limits.max().width as i32,
            &mut layout_lines,
            0,
            self.line.wrap_simple()
        );

        let mut width = 0;
        let mut height = 0;

        for layout_line in layout_lines {
            for glyph in layout_line.glyphs.iter() {
                width = cmp::max(width, (glyph.x + glyph.w) as i32 + 1);
            }
            height += self.metrics.line_height;
        }

        let size = Size::new(width as f32, height as f32);

        log::debug!("layout {:?} in {:?}", size, instant.elapsed());

        layout::Node::new(size)
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
        let instant = Instant::now();

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

        let shape = self.line.shape_opt().as_ref().unwrap();

        //TODO: can we cache this?
        let mut layout_lines = Vec::new();
        shape.layout(
            self.metrics.font_size,
            layout.bounds().width as i32,
            &mut layout_lines,
            0,
            self.line.wrap_simple()
        );

        let mut cache = state.cache.lock().unwrap();

        let mut line_y = self.metrics.font_size;
        for layout_line in layout_lines {
            for glyph in layout_line.glyphs.iter() {
                let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => text_color,
                };

                cache.with_pixels(cache_key, glyph_color, |x, y, color| {
                    let a = color.a();
                    if a > 0 {
                        let r = color.r();
                        let g = color.g();
                        let b = color.b();
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle::new(
                                    Point::new(
                                        layout.bounds().x + (x_int + x) as f32,
                                        layout.bounds().y + (line_y + y_int + y) as f32
                                    ),
                                    Size::new(1.0, 1.0)
                                ),
                                border_radius: 0.0,
                                border_width: 0.0,
                                border_color: Color::TRANSPARENT,
                            },
                            Color::from_rgba8(r, g, b, a as f32 / 255.0),
                        );
                    }
                });
            }
            line_y += self.metrics.line_height;
        }

        log::trace!("draw {:?} in {:?}", layout.bounds(), instant.elapsed());
    }
}

impl<'a, Message, Renderer> From<Text> for Element<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    fn from(text: Text) -> Self {
        Self::new(text)
    }
}

pub struct State {
    cache: Mutex<SwashCache<'static>>,
}

impl State {
    /// Creates a new [`State`].
    pub fn new() -> State {
        let instant = Instant::now();

        let state = State {
            cache: Mutex::new(SwashCache::new(&crate::FONT_SYSTEM)),
        };

        log::debug!("created state in {:?}", instant.elapsed());

        state
    }
}
