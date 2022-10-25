// SPDX-License-Identifier: MIT OR Apache-2.0

pub use fontdb::{Family, Stretch, Style, Weight};

pub struct Attrs<'a> {
    pub family: Family<'a>,
    pub monospaced: bool,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
}

impl<'a> Attrs<'a> {
    pub fn new() -> Self {
        Self {
            family: Family::SansSerif,
            monospaced: false,
            stretch: Stretch::Normal,
            style: Style::Normal,
            weight: Weight::NORMAL,
        }
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
}
