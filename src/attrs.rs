// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::borrow::Cow;
#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};
use core::ops::Range;

pub use fontdb::{Family, Stretch, Style, Weight};
use rangemap::RangeMap;

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
        Self(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    /// Get the red component
    #[inline]
    pub fn r(&self) -> u8 {
        ((self.0 & 0x00_FF_00_00) >> 16) as u8
    }

    /// Get the green component
    #[inline]
    pub fn g(&self) -> u8 {
        ((self.0 & 0x00_00_FF_00) >> 8) as u8
    }

    /// Get the blue component
    #[inline]
    pub fn b(&self) -> u8 {
        (self.0 & 0x00_00_00_FF) as u8
    }

    /// Get the alpha component
    #[inline]
    pub fn a(&self) -> u8 {
        ((self.0 & 0xFF_00_00_00) >> 24) as u8
    }
}

/// An owned version of [`Family`]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FamilyOwned {
    Name(Cow<'static, str>),
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl FamilyOwned {
    pub fn new(family: Family) -> Self {
        match family {
            Family::Name(name) => FamilyOwned::Name(Cow::Owned(name.to_string())),
            Family::Serif => FamilyOwned::Serif,
            Family::SansSerif => FamilyOwned::SansSerif,
            Family::Cursive => FamilyOwned::Cursive,
            Family::Fantasy => FamilyOwned::Fantasy,
            Family::Monospace => FamilyOwned::Monospace,
        }
    }

    pub fn as_family(&self) -> Family {
        match self {
            FamilyOwned::Name(name) => Family::Name(name),
            FamilyOwned::Serif => Family::Serif,
            FamilyOwned::SansSerif => Family::SansSerif,
            FamilyOwned::Cursive => Family::Cursive,
            FamilyOwned::Fantasy => Family::Fantasy,
            FamilyOwned::Monospace => Family::Monospace,
        }
    }
}

impl From<Family<'static>> for FamilyOwned {
    fn from(value: Family<'static>) -> Self {
        match value {
            Family::Name(name) => FamilyOwned::Name(Cow::Borrowed(name)),
            Family::Serif => FamilyOwned::Serif,
            Family::SansSerif => FamilyOwned::SansSerif,
            Family::Cursive => FamilyOwned::Cursive,
            Family::Fantasy => FamilyOwned::Fantasy,
            Family::Monospace => FamilyOwned::Monospace,
        }
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct AttrsBuilder(Attrs);

impl AttrsBuilder {
    pub fn new(attrs: Attrs) -> Self {
        Self(attrs)
    }

    pub fn build(self) -> Attrs {
        self.0
    }

    /// Set [Family]
    pub fn family(mut self, family: impl Into<FamilyOwned>) -> Self {
        self.0.family_owned = family.into();
        self
    }

    /// Set monospaced
    pub fn monospaced(mut self, monospaced: bool) -> Self {
        self.0.monospaced = monospaced;
        self
    }

    /// Set [Stretch]
    pub fn stretch(mut self, stretch: Stretch) -> Self {
        self.0.stretch = stretch;
        self
    }

    /// Set [Style]
    pub fn style(mut self, style: Style) -> Self {
        self.0.style = style;
        self
    }

    /// Set [Weight]
    pub fn weight(mut self, weight: Weight) -> Self {
        self.0.weight = weight;
        self
    }

    /// Set metadata
    pub fn metadata(mut self, metadata: usize) -> Self {
        self.0.metadata = metadata;
        self
    }
}

/// Text attributes
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Attrs {
    pub family_owned: FamilyOwned,
    pub monospaced: bool,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
    pub metadata: usize,
}

impl Attrs {
    /// Create a new set of attributes with sane defaults
    ///
    /// This defaults to a regular Sans-Serif font.
    pub fn new() -> Self {
        Self {
            family_owned: FamilyOwned::SansSerif,
            monospaced: false,
            stretch: Stretch::Normal,
            style: Style::Normal,
            weight: Weight::NORMAL,
            metadata: 0,
        }
    }

    pub fn builder() -> AttrsBuilder {
        AttrsBuilder::new(Self::new())
    }

    /// Check if font matches
    pub fn matches(&self, face: &fontdb::FaceInfo) -> bool {
        //TODO: smarter way of including emoji
        face.post_script_name.contains("Emoji")
            || (face.style == self.style
                && face.weight == self.weight
                && face.stretch == self.stretch
                && face.monospaced == self.monospaced)
    }

    /// Check if this set of attributes can be shaped with another
    pub fn compatible(&self, other: &Self) -> bool {
        self.family_owned == other.family_owned
            && self.monospaced == other.monospaced
            && self.stretch == other.stretch
            && self.style == other.style
            && self.weight == other.weight
    }
}

impl AsRef<Attrs> for Attrs {
    fn as_ref(&self) -> &Attrs {
        self
    }
}

impl From<&Attrs> for Attrs {
    fn from(value: &Attrs) -> Self {
        value.clone()
    }
}

#[derive(Eq, PartialEq)]
pub struct Spans<T>(RangeMap<usize, T>);

impl<T: Eq + Clone> Default for Spans<T> {
    fn default() -> Self {
        Self(RangeMap::default())
    }
}

impl<T: Eq + Clone> Spans<T> {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn add(&mut self, range: Range<usize>, attrs: impl Into<T>) {
        //do not support 1..1 even if by accident.
        if range.start == range.end {
            return;
        }

        self.0.insert(range, attrs.into());
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Range<usize>, &T)> {
        self.0.iter()
    }

    /// Get the attribute span for an index
    ///
    /// This returns a span that contains the index
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(&index)
    }

    /// Split at an offset
    pub fn split_off(&mut self, index: usize) -> Self {
        let mut new = Self::default();
        let mut removes = Vec::new();

        //get the keys we need to remove or fix.
        for span in self.0.iter() {
            if span.0.end <= index {
                continue;
            } else if span.0.start >= index {
                removes.push((span.0.clone(), false));
            } else {
                removes.push((span.0.clone(), true));
            }
        }

        for (key, resize) in removes {
            let (range, attrs) = self
                .0
                .get_key_value(&key.start)
                .map(|v| (v.0.clone(), v.1.clone()))
                .expect("attrs span not found");
            self.0.remove(key);

            if resize {
                new.0.insert(0..range.end - index, attrs.clone());
                self.0.insert(range.start..index, attrs);
            } else {
                new.0.insert(range.start - index..range.end - index, attrs);
            }
        }
        new
    }
}

/// List of text attributes to apply to a line
//TODO: have this clean up the spans when changes are made
#[derive(Eq, PartialEq)]
pub struct AttrsList {
    defaults: Attrs,
    spans: Spans<Attrs>,
}

impl AttrsList {
    /// Create a new attributes list with a set of default [Attrs]
    pub fn new(defaults: Attrs) -> Self {
        Self {
            defaults,
            spans: Spans::default(),
        }
    }

    /// Get the default [Attrs]
    pub fn defaults(&self) -> &Attrs {
        &self.defaults
    }

    /// Get the current attribute spans
    pub fn spans(&self) -> Vec<(&Range<usize>, &Attrs)> {
        self.spans.iter().collect()
    }

    /// Clear the current attribute spans
    pub fn clear_spans(&mut self) {
        self.spans.clear();
    }

    /// Add an attribute span, removes any previous matching parts of spans
    pub fn add_span(&mut self, range: Range<usize>, attrs: impl Into<Attrs>) {
        self.spans.add(range, attrs.into());
    }

    /// Get the attribute span for an index
    ///
    /// This returns a span that contains the index
    pub fn get_span(&self, index: usize) -> &Attrs {
        self.spans.get(index).unwrap_or(&self.defaults)
    }

    /// Split attributes list at an offset
    pub fn split_off(&mut self, index: usize) -> Self {
        Self {
            defaults: self.defaults.clone(),
            spans: self.spans.split_off(index),
        }
    }
}
