// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ops::Range;

pub use fontdb::{Family, Stretch, Style, Weight};

/// Text color
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FamilyOwned {
    Name(String),
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl FamilyOwned {
    pub fn new(family: Family) -> Self {
        match family {
            Family::Name(name) => FamilyOwned::Name(name.to_string()),
            Family::Serif => FamilyOwned::Serif,
            Family::SansSerif => FamilyOwned::SansSerif,
            Family::Cursive => FamilyOwned::Cursive,
            Family::Fantasy => FamilyOwned::Fantasy,
            Family::Monospace => FamilyOwned::Monospace,
        }
    }

    pub fn as_family(&self) -> Family {
        match self {
            FamilyOwned::Name(name) => Family::Name(&name),
            FamilyOwned::Serif => Family::Serif,
            FamilyOwned::SansSerif => Family::SansSerif,
            FamilyOwned::Cursive => Family::Cursive,
            FamilyOwned::Fantasy => Family::Fantasy,
            FamilyOwned::Monospace => Family::Monospace,
        }
    }
}

/// Text attributes
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
    /// Create a new set of attributes with sane defaults
    ///
    /// This defaults to a regular Sans-Serif font.
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

    /// Set [Color]
    pub fn color(mut self, color: Color) -> Self {
        self.color_opt = Some(color);
        self
    }

    /// Set [Family]
    pub fn family(mut self, family: Family<'a>) -> Self {
        self.family = family;
        self
    }

    /// Set monospaced
    pub fn monospaced(mut self, monospaced: bool) -> Self {
        self.monospaced = monospaced;
        self
    }

    /// Set [Stretch]
    pub fn stretch(mut self, stretch: Stretch) -> Self {
        self.stretch = stretch;
        self
    }

    /// Set [Style]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set [Weight]
    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    /// Check if font matches
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

    /// Check if this set of attributes can be shaped with another
    pub fn compatible(&self, other: &Self) -> bool {
        self.family == other.family
        && self.monospaced == other.monospaced
        && self.stretch == other.stretch
        && self.style == other.style
        && self.weight == other.weight
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AttrsOwned {
    //TODO: should this be an option?
    pub color_opt: Option<Color>,
    pub family_owned: FamilyOwned,
    pub monospaced: bool,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
}

impl AttrsOwned {
    pub fn new(attrs: Attrs) -> Self {
        Self {
            color_opt: attrs.color_opt,
            family_owned: FamilyOwned::new(attrs.family),
            monospaced: attrs.monospaced,
            stretch: attrs.stretch,
            style: attrs.style,
            weight: attrs.weight,
        }
    }

    pub fn as_attrs(&self) -> Attrs {
        Attrs {
            color_opt: self.color_opt,
            family: self.family_owned.as_family(),
            monospaced: self.monospaced,
            stretch: self.stretch,
            style: self.style,
            weight: self.weight,
        }
    }
}

/// List of text attributes to apply to a line
//TODO: have this clean up the spans when changes are made
#[derive(Eq, PartialEq)]
pub struct AttrsList {
    defaults: AttrsOwned,
    spans: Vec<(Range<usize>, AttrsOwned)>,
}

impl AttrsList {
    /// Create a new attributes list with a set of default [Attrs]
    pub fn new(defaults: Attrs) -> Self {
        Self {
            defaults: AttrsOwned::new(defaults),
            spans: Vec::new(),
        }
    }

    /// Get the default [Attrs]
    pub fn defaults(&self) -> Attrs {
        self.defaults.as_attrs()
    }

    /// Get the current attribute spans
    pub fn spans(&self) -> &Vec<(Range<usize>, AttrsOwned)> {
        &self.spans
    }

    /// Clear the current attribute spans
    pub fn clear_spans(&mut self) {
        self.spans.clear();
    }

    /// Add an attribute span, removes any previous matching parts of spans
    pub fn add_span(&mut self, range: Range<usize>, attrs: Attrs) {
        //do not support 1..1 even if by accident.
        if range.start == range.end {
            return;
        }

        let mut rework_spans = Vec::with_capacity(3);
        let mut i = 0;

        //Grab intersecting parts that are not fully intersected. remove those that are.
        //This clips or splits the parts that are outside of the range.
        while i < self.spans.len() {
            if self.spans[i].0.end <= range.end && self.spans[i].0.start >= range.start {
                let _ = self.spans.remove(i);
            } else if self.spans[i].0.end > range.end && self.spans[i].0.start >= range.start && self.spans[i].0.start <= range.end {
                let rework = self.spans.remove(i);
                rework_spans.push((range.end..rework.0.end, rework.1))
            } else if self.spans[i].0.end <= range.end && self.spans[i].0.end >= range.start && self.spans[i].0.start < range.start {
                let rework = self.spans.remove(i);
                rework_spans.push((rework.0.start..range.start, rework.1))
            } else if self.spans[i].0.end > range.end && self.spans[i].0.start < range.start {
                let rework = self.spans.remove(i);
                rework_spans.push((rework.0.start..range.start, rework.1.clone()));
                rework_spans.push((range.end..rework.0.end, rework.1));
            } else if self.spans[i].0.start > range.end {
                break;
            } else {
                i += 1;
            }
        }

        // Readd reworked arrays back.
        for reworked in rework_spans {
            self.spans.push(reworked);
        }

        // Combine span if possible
        let mut combined = false;
        for span in self.spans.iter_mut() {
            if span.1.as_attrs() != attrs {
                // Ignore not matching attrs
                continue;
            }

            if span.0.end == range.start {
                // Extend span with range at end
                span.0.end = range.end;
                combined = true;
                break;
            }

            if span.0.start == range.end {
                // Extend span with range at start
                span.0.start = range.start;
                combined = true;
                break;
            }
        }

        if ! combined {
            //Finally lets add the new span. it should fit now.
            self.spans.push((range, AttrsOwned::new(attrs)));
        }

        //sort by start to speed up further additions
        self.spans.sort_by(|a, b| a.0.start.partial_cmp(&b.0.start).unwrap());
    }

    /// Get the highest priority attribute span for a range
    ///
    /// This returns the first span that contains the range
    pub fn get_span(&self, range: Range<usize>) -> Attrs {
        for span in self.spans.iter() {
            if range.start >= span.0.start && range.end <= span.0.end {
                return span.1.as_attrs();
            }
        }
        self.defaults.as_attrs()
    }

    /// Split attributes list at an offset
    pub fn split_off(&mut self, index: usize) -> Self {
        let mut new = Self::new(self.defaults.as_attrs());
        let mut i = 0;
        while i < self.spans.len() {
            if self.spans[i].0.end <= index {
                // Leave this in the previous attributes list
                i += 1;
            } else if self.spans[i].0.start >= index {
                // Move this to the new attributes list
                let (range, attrs) = self.spans.remove(i);
                new.spans.push((
                    range.start - index..range.end - index,
                    attrs
                ));
            } else {
                // New span has index..end
                new.spans.push((
                    0..self.spans[i].0.end - index,
                    self.spans[i].1.clone()
                ));
                // Old span has start..index
                self.spans[i].0.end = index;
                i += 1;
            }
        }
        new
    }
}
