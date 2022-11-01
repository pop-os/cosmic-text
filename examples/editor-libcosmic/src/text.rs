// SPDX-License-Identifier: MIT OR Apache-2.0

use cosmic::iced_native::{
    {Color, Element, Length, Point, Rectangle, Size, Theme},
    image,
    layout::{self, Layout},
    renderer,
    widget::{self, tree, Widget},
};
use cosmic_text::{
    Attrs,
    AttrsList,
    SwashCache,
    BufferLine,
    Metrics,
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
    line: BufferLine<'static>,
    metrics: Metrics,
}

impl Text {
    pub fn new(string: &str) -> Self {
        let instant = Instant::now();

        //TODO: make it possible to set attrs
        let mut line = BufferLine::new(
            string,
            AttrsList::new(Attrs::new())
        );

        //TODO: do we have to immediately shape?
        line.shape(&crate::FONT_SYSTEM);

        let text = Self {
            line,
            metrics: Metrics::new(14, 20),
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
        let layout_lines = shape.layout(
            self.metrics.font_size,
            limits.max().width as i32,
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

        let layout_w = layout.bounds().width as i32;
        let layout_h = layout.bounds().height as i32;

        let shape = self.line.shape_opt().as_ref().unwrap();

        //TODO: can we cache this?
        let layout_lines = shape.layout(
            self.metrics.font_size,
            layout_w,
            self.line.wrap_simple()
        );

        let mut cache = state.cache.lock().unwrap();

        let mut pixels = vec![0; layout_w as usize * layout_h as usize * 4];

        let mut line_y = self.metrics.font_size;
        for layout_line in layout_lines {
            for glyph in layout_line.glyphs.iter() {
                let (cache_key, x_int, y_int) = (glyph.cache_key, glyph.x_int, glyph.y_int);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => text_color,
                };

                cache.with_pixels(cache_key, glyph_color, |pixel_x, pixel_y, color| {
                    let x = x_int + pixel_x;
                    let y = line_y + y_int + pixel_y;
                    draw_pixel(&mut pixels, layout_w, layout_h, x, y, color);
                });
            }
            line_y += self.metrics.line_height;
        }

        let handle = image::Handle::from_pixels(layout_w as u32, layout_h as u32, pixels);
        image::Renderer::draw(renderer, handle, layout.bounds());

        log::trace!("draw {:?} in {:?}", layout.bounds(), instant.elapsed());
    }
}

pub fn draw_pixel(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    color: cosmic_text::Color
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
    
    let mut current =
        buffer[offset] as u32 |
        (buffer[offset + 1] as u32) << 8 |
        (buffer[offset + 2] as u32) << 16 |
        (buffer[offset + 3] as u32) << 24;
    
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
    
    buffer[offset] = current as u8;
    buffer[offset + 1] = (current >> 8) as u8;
    buffer[offset + 2] = (current >> 16) as u8;
    buffer[offset + 3] = (current >> 24) as u8;
}

impl<'a, Message, Renderer> From<Text> for Element<'a, Message, Renderer>
where
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle>,
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
