// SPDX-License-Identifier: MIT OR Apache-2.0

pub use fontdb::{Family, Stretch, Style, Weight};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Color(pub u32);

impl Color {
    /// Create new color with red, green, and blue components
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 0xFF)
    }

    /// Create new color with red, green, blue, and alpha components
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(
            ((a as u32) << 24) |
            ((r as u32) << 16) |
            ((g as u32) << 8) |
            (b as u32)
        )
    }

    /// Get the red component
    #[inline]
    pub fn r(&self) -> u8 {
        ((self.0 & 0x00FF0000) >> 16) as u8
    }

    /// Get the green component
    #[inline]
    pub fn g(&self) -> u8 {
        ((self.0 & 0x0000FF00) >> 8) as u8
    }

    /// Get the blue component
    #[inline]
    pub fn b(&self) -> u8 {
        (self.0 & 0x000000FF) as u8
    }

    /// Get the alpha component
    #[inline]
    pub fn a(&self) -> u8 {
        ((self.0 & 0xFF000000) >> 24) as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Attrs<'a> {
    //TODO: should this be an option?
    pub color_opt: Option<Color>,
    pub family: Family<'a>,
    pub monospaced: bool,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
}

impl<'a> Attrs<'a> {
    pub fn new() -> Self {
        Self {
            color_opt: None,
            family: Family::SansSerif,
            monospaced: false,
            stretch: Stretch::Normal,
            style: Style::Normal,
            weight: Weight::NORMAL,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color_opt = Some(color);
        self
    }

    pub fn family(mut self, family: Family<'a>) -> Self {
        self.family = family;
        self
    }

    pub fn monospaced(mut self, monospaced: bool) -> Self {
        self.monospaced = monospaced;
        self
    }

    pub fn stretch(mut self, stretch: Stretch) -> Self {
        self.stretch = stretch;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    pub fn matches(&self, face: &fontdb::FaceInfo) -> bool {
        //TODO: smarter way of including emoji
        face.post_script_name.contains("Emoji") ||
        (
            face.style == self.style &&
            face.weight == self.weight &&
            face.stretch == self.stretch &&
            face.monospaced == self.monospaced
        )
    }

    pub fn compatible(&self, other: &Self) -> bool {
        self.family == other.family
        && self.monospaced == other.monospaced
        && self.stretch == other.stretch
        && self.style == other.style
        && self.weight == other.weight
    }
}

#[derive(Eq, PartialEq)]
pub struct AttrsList<'a> {
    defaults: Attrs<'a>,
    spans: Vec<(usize, usize, Attrs<'a>)>,
}

impl<'a> AttrsList<'a> {
    pub fn new(defaults: Attrs<'a>) -> Self {
        Self {
            defaults,
            spans: Vec::new(),
        }
    }

    pub fn defaults(&self) -> Attrs<'a> {
        self.defaults
    }

    pub fn spans(&self) -> &Vec<(usize, usize, Attrs<'a>)> {
        &self.spans
    }

    pub fn clear_spans(&mut self) {
        self.spans.clear();
    }

    pub fn add_span(&mut self, start: usize, end: usize, attrs: Attrs<'a>) {
        self.spans.push((start, end, attrs));
    }

    pub fn get_span(&self, start: usize, end: usize) -> Attrs<'a> {
        let mut attrs = self.defaults;
        for span in self.spans.iter() {
            if start >= span.0 && end <= span.1 {
                attrs = span.2;
            }
        }
        attrs
    }
}
