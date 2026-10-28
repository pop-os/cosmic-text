// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::ops::Range;
use rangemap::RangeMap;
use smol_str::SmolStr;

use crate::{CacheKeyFlags, Metrics};

pub use fontdb::{Family, Stretch, Style, Weight};

/// Text color
#[derive(Clone, Copy, Debug, PartialOrd, Ord, Eq, Hash, PartialEq)]
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

    /// Get a tuple over all of the attributes, in `(r, g, b, a)` order.
    #[inline]
    pub const fn as_rgba_tuple(self) -> (u8, u8, u8, u8) {
        (self.r(), self.g(), self.b(), self.a())
    }

    /// Get an array over all of the components, in `[r, g, b, a]` order.
    #[inline]
    pub const fn as_rgba(self) -> [u8; 4] {
        [self.r(), self.g(), self.b(), self.a()]
    }

    /// Get the red component
    #[inline]
    pub const fn r(&self) -> u8 {
        ((self.0 & 0x00_FF_00_00) >> 16) as u8
    }

    /// Get the green component
    #[inline]
    pub const fn g(&self) -> u8 {
        ((self.0 & 0x00_00_FF_00) >> 8) as u8
    }

    /// Get the blue component
    #[inline]
    pub const fn b(&self) -> u8 {
        (self.0 & 0x00_00_00_FF) as u8
    }

    /// Get the alpha component
    #[inline]
    pub const fn a(&self) -> u8 {
        ((self.0 & 0xFF_00_00_00) >> 24) as u8
    }
}

/// An owned version of [`Family`]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FamilyOwned {
    Name(SmolStr),
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl FamilyOwned {
    pub fn new(family: Family) -> Self {
        match family {
            Family::Name(name) => Self::Name(SmolStr::from(name)),
            Family::Serif => Self::Serif,
            Family::SansSerif => Self::SansSerif,
            Family::Cursive => Self::Cursive,
            Family::Fantasy => Self::Fantasy,
            Family::Monospace => Self::Monospace,
        }
    }

    pub fn as_family(&self) -> Family<'_> {
        match self {
            Self::Name(name) => Family::Name(name),
            Self::Serif => Family::Serif,
            Self::SansSerif => Family::SansSerif,
            Self::Cursive => Family::Cursive,
            Self::Fantasy => Family::Fantasy,
            Self::Monospace => Family::Monospace,
        }
    }
}

/// Metrics, but implementing Eq and Hash using u32 representation of f32
//TODO: what are the edge cases of this?
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CacheMetrics {
    font_size_bits: u32,
    line_height_bits: u32,
}

impl From<Metrics> for CacheMetrics {
    fn from(metrics: Metrics) -> Self {
        Self {
            font_size_bits: metrics.font_size.to_bits(),
            line_height_bits: metrics.line_height.to_bits(),
        }
    }
}

impl From<CacheMetrics> for Metrics {
    fn from(metrics: CacheMetrics) -> Self {
        Self {
            font_size: f32::from_bits(metrics.font_size_bits),
            line_height: f32::from_bits(metrics.line_height_bits),
        }
    }
}
/// A 4-byte `OpenType` feature tag identifier
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FeatureTag([u8; 4]);

impl FeatureTag {
    pub const fn new(tag: &[u8; 4]) -> Self {
        Self(*tag)
    }

    /// Kerning adjusts spacing between specific character pairs
    pub const KERNING: Self = Self::new(b"kern");
    /// Standard ligatures (fi, fl, etc.)
    pub const STANDARD_LIGATURES: Self = Self::new(b"liga");
    /// Contextual ligatures (context-dependent ligatures)
    pub const CONTEXTUAL_LIGATURES: Self = Self::new(b"clig");
    /// Contextual alternates (glyph substitutions based on context)
    pub const CONTEXTUAL_ALTERNATES: Self = Self::new(b"calt");
    /// Discretionary ligatures (optional stylistic ligatures)
    pub const DISCRETIONARY_LIGATURES: Self = Self::new(b"dlig");
    /// Small caps (lowercase to small capitals)
    pub const SMALL_CAPS: Self = Self::new(b"smcp");
    /// All small caps (uppercase and lowercase to small capitals)
    pub const ALL_SMALL_CAPS: Self = Self::new(b"c2sc");
    /// Stylistic Set 1 (font-specific alternate glyphs)
    pub const STYLISTIC_SET_1: Self = Self::new(b"ss01");
    /// Stylistic Set 2 (font-specific alternate glyphs)
    pub const STYLISTIC_SET_2: Self = Self::new(b"ss02");

    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Feature {
    pub tag: FeatureTag,
    pub value: u32,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct FontFeatures {
    features: Vec<Feature>,
}

impl FontFeatures {
    pub const fn new() -> Self {
        Self {
            features: Vec::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.features.iter()
    }

    /// Set a feature value by [`FeatureTag`].
    ///
    /// This ensures that the features are sorted by [`FeatureTag`] without duplicate tags.
    pub fn set(&mut self, tag: FeatureTag, value: u32) -> &mut Self {
        match self
            .features
            .binary_search_by_key(&tag, |feature| feature.tag)
        {
            Ok(index) => self.features[index].value = value,
            Err(index) => self.features.insert(index, Feature { tag, value }),
        }
        self
    }

    /// Enable a feature (set to 1)
    pub fn enable(&mut self, tag: FeatureTag) -> &mut Self {
        self.set(tag, 1)
    }

    /// Disable a feature (set to 0)
    pub fn disable(&mut self, tag: FeatureTag) -> &mut Self {
        self.set(tag, 0)
    }
}

/// A wrapper for letter spacing to get around that f32 doesn't implement Eq and Hash
#[derive(Clone, Copy, Debug)]
pub struct LetterSpacing(pub f32);

impl PartialEq for LetterSpacing {
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_nan() {
            other.0.is_nan()
        } else {
            self.0 == other.0
        }
    }
}

impl Eq for LetterSpacing {}

impl Hash for LetterSpacing {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        const CANONICAL_NAN_BITS: u32 = 0x7fc0_0000;

        let bits = if self.0.is_nan() {
            CANONICAL_NAN_BITS
        } else {
            // Add +0.0 to canonicalize -0.0 to +0.0
            (self.0 + 0.0).to_bits()
        };

        bits.hash(hasher);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    // TODO: Wavy
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextDecoration {
    pub underline: UnderlineStyle,
    pub underline_color_opt: Option<Color>,
    pub strikethrough: bool,
    pub strikethrough_color_opt: Option<Color>,
    pub overline: bool,
    pub overline_color_opt: Option<Color>,
}

impl TextDecoration {
    pub const fn new() -> Self {
        Self {
            underline: UnderlineStyle::None,
            underline_color_opt: None,
            strikethrough: false,
            strikethrough_color_opt: None,
            overline: false,
            overline_color_opt: None,
        }
    }

    pub const fn has_decoration(&self) -> bool {
        !matches!(self.underline, UnderlineStyle::None) || self.strikethrough || self.overline
    }
}

/// Size of an inline box, using u32 bit storage for Eq + Hash (same as `CacheMetrics`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InlineBoxSize {
    width_bits: u32,
    ascent_bits: u32,
    descent_bits: u32,
    /// [`InlineResolve::None`] means `ascent`/`descent` are final.
    resolve: InlineResolve,
}

/// How an inline box should be resolved against surrounding text extents at
/// layout time. `ascent`/`descent` stored on [`InlineBoxSize`] are placeholders
/// for [`InlineResolve::Top`] / [`InlineResolve::Bottom`], the real values are
/// computed once the enclosing line's text metrics are known.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum InlineResolve {
    None,
    Top,
    Bottom,
    Center,
}

/// Vertical alignment mode for an inline box with a given width/height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InlineAlignment {
    /// The box baseline is inside the rectangle at `offset` px from the top.
    /// `offset` is clamped to `[0, height]`.
    ///
    /// - Baseline {offset:0} -> BelowBaseline
    /// - Baseline {offset:height} -> AboveBaseline
    /// - Baseline {offset:height/2} -> Middle
    Baseline { offset: f32 },
    /// The bottom edge of the box sits on the text baseline.
    AboveBaseline,
    /// The top edge of the box sits on the text baseline.
    BelowBaseline,
    /// The box is centered around the text baseline.
    Middle,
    /// The top edge of the box sits flush with the top of the tallest text
    /// (the line's `max_text_ascent`) in the surrounding line.
    /// TODO: should this be the paragraph? or the whole text?
    ///
    /// Resolved at layout time. unlike [`InlineAlignment::AboveBaseline`], the box can
    /// extend below the baseline when its height exceeds the text ascent.
    TextTop,
    /// Opposite of [`Top`]
    TextBottom,
    /// The box is centered on the surrounding text's vertical extent, midway
    /// between the line's `max_text_ascent` and `max_text_descent`. Resolved at
    /// layout time, falls back to [`InlineAlignment::Middle`] when the line has
    /// no text. Prefer this over `Middle` to center icons on the text.
    TextCenter,
}

impl InlineBoxSize {
    fn is_valid_dimension(value: f32) -> bool {
        value.is_finite() && value >= 0.0
    }

    fn sanitize_dimension(value: f32) -> f32 {
        if Self::is_valid_dimension(value) {
            value
        } else {
            0.0
        }
    }

    fn sanitize_baseline_offset(offset: f32, height: f32) -> f32 {
        if offset.is_finite() {
            offset.clamp(0.0, height)
        } else {
            0.0
        }
    }

    fn split_height(height: f32, alignment: InlineAlignment) -> (f32, f32) {
        match alignment {
            InlineAlignment::Baseline { offset } => {
                let ascent = Self::sanitize_baseline_offset(offset, height);
                (ascent, height - ascent)
            }
            InlineAlignment::AboveBaseline => (height, 0.0),
            InlineAlignment::BelowBaseline => (0.0, height),
            InlineAlignment::TextTop => (height, 0.0),
            InlineAlignment::TextBottom => (0.0, height),
            InlineAlignment::Middle | InlineAlignment::TextCenter => {
                let ascent = height * 0.5;
                (ascent, height - ascent)
            }
        }
    }

    /// Create a new inline box size from width, total height and an alignment mode.
    ///
    /// Invalid values (`NaN`, infinite, or negative) are debug-asserted and
    /// sanitized to `0.0` in release builds.
    pub fn new(width: f32, height: f32, alignment: InlineAlignment) -> Self {
        debug_assert!(
            Self::is_valid_dimension(width) && Self::is_valid_dimension(height),
            "InlineBoxSize values must be finite and non-negative"
        );
        if let InlineAlignment::Baseline { offset } = alignment {
            debug_assert!(
                offset.is_finite() && offset >= 0.0 && offset <= height,
                "InlineAlignment::Baseline offset must be finite and in [0, height]"
            );
        }

        let width = Self::sanitize_dimension(width);
        let height = Self::sanitize_dimension(height);
        let (ascent, descent) = Self::split_height(height, alignment);
        let resolve = match alignment {
            InlineAlignment::TextTop => InlineResolve::Top,
            InlineAlignment::TextBottom => InlineResolve::Bottom,
            InlineAlignment::TextCenter => InlineResolve::Center,
            _ => InlineResolve::None,
        };
        Self {
            width_bits: width.to_bits(),
            ascent_bits: ascent.to_bits(),
            descent_bits: descent.to_bits(),
            resolve,
        }
    }

    /// Total height of the box in pixels.
    pub fn height(&self) -> f32 {
        self.ascent() + self.descent()
    }

    pub(crate) fn resolve(&self) -> InlineResolve {
        self.resolve
    }

    /// Create a new inline box size from width, total height and alignment mode.
    ///
    /// Returns `None` for invalid values.
    pub fn try_new(width: f32, height: f32, alignment: InlineAlignment) -> Option<Self> {
        if !Self::is_valid_dimension(width) || !Self::is_valid_dimension(height) {
            return None;
        }
        if let InlineAlignment::Baseline { offset } = alignment {
            if !offset.is_finite() || offset < 0.0 || offset > height {
                return None;
            }
        }

        Some(Self::new(width, height, alignment))
    }

    /// Width in pixels
    pub fn width(&self) -> f32 {
        f32::from_bits(self.width_bits)
    }

    /// Ascent (above baseline) in pixels.
    ///
    /// For [`InlineAlignment::Top`], [`InlineAlignment::Bottom`] and
    /// [`InlineAlignment::Center`], this is a preliminary value. If you need the final layout positions you should
    /// read [`InlineBoxSpan`] on a laid-out [`LayoutRun`] instead.
    pub fn ascent(&self) -> f32 {
        f32::from_bits(self.ascent_bits)
    }

    /// See [`Self::ascent`] for notes on [`InlineAlignment::Top`] /
    /// [`InlineAlignment::Bottom`] / [`InlineAlignment::Center`].
    pub fn descent(&self) -> f32 {
        f32::from_bits(self.descent_bits)
    }
}

/// Identifier for an inline box.
///
/// Wraps a `u64` so callers can't accidentally pass another integer. Build one
/// with [`InlineBoxId::new`] or `InlineBoxId::from(u64)`. For collision-free
/// ids, mint them via [`Buffer::next_inline_box_id`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InlineBoxId(u64);

impl InlineBoxId {
    /// Create a new `InlineBoxId` from a raw `u64`.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Access the underlying raw `u64`.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for InlineBoxId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<InlineBoxId> for u64 {
    fn from(id: InlineBoxId) -> Self {
        id.0
    }
}

impl core::fmt::Display for InlineBoxId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl PartialEq<u64> for InlineBoxId {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl PartialEq<InlineBoxId> for u64 {
    fn eq(&self, other: &InlineBoxId) -> bool {
        *self == other.0
    }
}

/// An inline box placeholder to reserve space for non-text elements (images,
/// icons, widgets).
///
/// # How it works:
///
/// The shaping algorithm creates inline boxes as synthetic glyphs
/// whose `font_id == fontdb::ID::dummy()`. When iterating a [`LayoutRun`]'s
/// glyphs for rendering text you **MUST** skip dummy-font glyphs.
///
/// To render the inline box content use one of:
///
/// - Override [`Renderer::inline_box`], which the built-in render dispatches once per box
/// - Iterate [`LayoutRun::inline_boxes`] directly and draw the box content by `id`.
///
/// # IDs
///
/// `id` is a caller-assigned [`usize`] used to look up what to draw.
/// The library does not enforce uniqueness. If two boxes share an id, renderers
/// that dispatch by id will be ambiguous.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InlineBox {
    /// Identifier used to match the placeholder back to the content
    ///
    /// IMPORTANT: Uniqueness is caller-enforced (use [`Buffer::next_inline_box_id`] for collision-free ids)
    pub id: InlineBoxId,
    /// Size of the inline box in pixels
    pub size: InlineBoxSize,
}

impl InlineBox {
    /// Create an inline box from width/height and alignment mode.
    pub fn new(
        id: impl Into<InlineBoxId>,
        width: f32,
        height: f32,
        alignment: InlineAlignment,
    ) -> Self {
        Self {
            id: id.into(),
            size: InlineBoxSize::new(width, height, alignment),
        }
    }

    /// Fallible version of [`Self::new`].
    pub fn try_new(
        id: impl Into<InlineBoxId>,
        width: f32,
        height: f32,
        alignment: InlineAlignment,
    ) -> Option<Self> {
        InlineBoxSize::try_new(width, height, alignment).map(|size| Self {
            id: id.into(),
            size,
        })
    }
}

/// Offset and thickness for a text decoration line, in EM units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DecorationMetrics {
    /// Offset from baseline in EM units
    pub offset: f32,
    /// Thickness in EM units
    pub thickness: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphDecorationData {
    /// The text decoration configuration from the user
    pub text_decoration: TextDecoration,
    /// Underline offset and thickness from the font
    pub underline_metrics: DecorationMetrics,
    /// Strikethrough offset and thickness from the font
    pub strikethrough_metrics: DecorationMetrics,
    /// Font ascent in EM units (ascent / upem).
    /// Used for overline positioning
    pub ascent: f32,
}

/// Text attributes
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Attrs<'a> {
    //TODO: should this be an option?
    pub color_opt: Option<Color>,
    pub family: Family<'a>,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
    pub metadata: usize,
    pub cache_key_flags: CacheKeyFlags,
    pub metrics_opt: Option<CacheMetrics>,
    /// Letter spacing (tracking) in EM
    pub letter_spacing_opt: Option<LetterSpacing>,
    pub font_features: FontFeatures,
    pub text_decoration: TextDecoration,
    pub inline_box: Option<InlineBox>,
}

impl<'a> Attrs<'a> {
    /// Create a new set of attributes with sane defaults
    ///
    /// This defaults to a regular Sans-Serif font.
    pub const fn new() -> Self {
        Self {
            color_opt: None,
            family: Family::SansSerif,
            stretch: Stretch::Normal,
            style: Style::Normal,
            weight: Weight::NORMAL,
            metadata: 0,
            cache_key_flags: CacheKeyFlags::empty(),
            metrics_opt: None,
            letter_spacing_opt: None,
            font_features: FontFeatures::new(),
            text_decoration: TextDecoration::new(),
            inline_box: None,
        }
    }

    /// Set [Color]
    pub const fn color(mut self, color: Color) -> Self {
        self.color_opt = Some(color);
        self
    }

    /// Set [Family]
    pub const fn family(mut self, family: Family<'a>) -> Self {
        self.family = family;
        self
    }

    /// Set [Stretch]
    pub const fn stretch(mut self, stretch: Stretch) -> Self {
        self.stretch = stretch;
        self
    }

    /// Set [Style]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set [Weight]
    pub const fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    /// Set metadata
    pub const fn metadata(mut self, metadata: usize) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set [`CacheKeyFlags`]
    pub const fn cache_key_flags(mut self, cache_key_flags: CacheKeyFlags) -> Self {
        self.cache_key_flags = cache_key_flags;
        self
    }

    /// Set [`Metrics`], overriding values in buffer
    pub fn metrics(mut self, metrics: Metrics) -> Self {
        self.metrics_opt = Some(metrics.into());
        self
    }

    /// Set letter spacing (tracking) in EM
    pub const fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing_opt = Some(LetterSpacing(letter_spacing));
        self
    }

    /// Set [`FontFeatures`]
    pub fn font_features(mut self, font_features: FontFeatures) -> Self {
        self.font_features = font_features;
        self
    }

    pub const fn underline(mut self, style: UnderlineStyle) -> Self {
        self.text_decoration.underline = style;
        self
    }

    pub const fn underline_color(mut self, color: Color) -> Self {
        self.text_decoration.underline_color_opt = Some(color);
        self
    }

    pub const fn strikethrough(mut self) -> Self {
        self.text_decoration.strikethrough = true;
        self
    }

    pub const fn strikethrough_color(mut self, color: Color) -> Self {
        self.text_decoration.strikethrough_color_opt = Some(color);
        self
    }

    pub const fn overline(mut self) -> Self {
        self.text_decoration.overline = true;
        self
    }

    pub const fn overline_color(mut self, color: Color) -> Self {
        self.text_decoration.overline_color_opt = Some(color);
        self
    }

    /// Attach an [`InlineBox`] placeholder to this [`Attrs`].
    ///
    /// The shaper only creates a box when it encounters a `U+FFFC` Object
    /// Replacement Character whose attributes carry this field. In practice,
    /// prefer [`Buffer::set_rich_text_with_boxes`] and
    /// [`RichSpan::InlineBox`], which handle everything for you.
    /// Call this directly only when building a custom [`AttrsList`] by hand.
    pub const fn inline_box(mut self, ib: InlineBox) -> Self {
        self.inline_box = Some(ib);
        self
    }

    /// Check if this set of attributes can be shaped with another
    pub fn compatible(&self, other: &Self) -> bool {
        self.family == other.family
            && self.stretch == other.stretch
            && self.style == other.style
            && self.weight == other.weight
            && self.font_features == other.font_features
    }
}

/// Font-specific part of [`Attrs`] to be used for matching
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FontMatchAttrs {
    family: FamilyOwned,
    stretch: Stretch,
    style: Style,
    weight: Weight,
}

impl<'a> From<&Attrs<'a>> for FontMatchAttrs {
    fn from(attrs: &Attrs<'a>) -> Self {
        Self {
            family: FamilyOwned::new(attrs.family),
            stretch: attrs.stretch,
            style: attrs.style,
            weight: attrs.weight,
        }
    }
}

/// An owned version of [`Attrs`]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AttrsOwned {
    //TODO: should this be an option?
    pub color_opt: Option<Color>,
    pub family_owned: FamilyOwned,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
    pub metadata: usize,
    pub cache_key_flags: CacheKeyFlags,
    pub metrics_opt: Option<CacheMetrics>,
    /// Letter spacing (tracking) in EM
    pub letter_spacing_opt: Option<LetterSpacing>,
    pub font_features: FontFeatures,
    pub text_decoration: TextDecoration,
    pub inline_box: Option<InlineBox>,
}

impl AttrsOwned {
    /// Equal in every attribute that affects shaping, ignoring
    /// `text_decoration`, which is resolved at layout.
    pub fn eq_shape_attrs(&self, other: &Self) -> bool {
        self.color_opt == other.color_opt
            && self.family_owned == other.family_owned
            && self.stretch == other.stretch
            && self.style == other.style
            && self.weight == other.weight
            && self.metadata == other.metadata
            && self.cache_key_flags == other.cache_key_flags
            && self.metrics_opt == other.metrics_opt
            && self.letter_spacing_opt == other.letter_spacing_opt
            && self.font_features == other.font_features
    }

    pub fn new(attrs: &Attrs) -> Self {
        Self {
            color_opt: attrs.color_opt,
            family_owned: FamilyOwned::new(attrs.family),
            stretch: attrs.stretch,
            style: attrs.style,
            weight: attrs.weight,
            metadata: attrs.metadata,
            cache_key_flags: attrs.cache_key_flags,
            metrics_opt: attrs.metrics_opt,
            letter_spacing_opt: attrs.letter_spacing_opt,
            font_features: attrs.font_features.clone(),
            text_decoration: attrs.text_decoration,
            inline_box: attrs.inline_box,
        }
    }

    pub fn as_attrs(&self) -> Attrs<'_> {
        Attrs {
            color_opt: self.color_opt,
            family: self.family_owned.as_family(),
            stretch: self.stretch,
            style: self.style,
            weight: self.weight,
            metadata: self.metadata,
            cache_key_flags: self.cache_key_flags,
            metrics_opt: self.metrics_opt,
            letter_spacing_opt: self.letter_spacing_opt,
            font_features: self.font_features.clone(),
            text_decoration: self.text_decoration,
            inline_box: self.inline_box,
        }
    }
}

/// List of text attributes to apply to a line
//TODO: have this clean up the spans when changes are made
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AttrsList {
    defaults: AttrsOwned,
    pub(crate) spans: RangeMap<usize, AttrsOwned>,
}

impl AttrsList {
    /// Create a new attributes list with a set of default [Attrs]
    pub fn new(defaults: &Attrs) -> Self {
        Self {
            defaults: AttrsOwned::new(defaults),
            spans: RangeMap::new(),
        }
    }

    /// Get the default [Attrs]
    pub fn defaults(&self) -> Attrs<'_> {
        self.defaults.as_attrs()
    }

    /// True if the two lists differ only in `text_decoration`. Spans that carry
    /// only decoration (otherwise equal to the defaults) are ignored.
    pub fn eq_shape_attrs(&self, other: &Self) -> bool {
        if !self.defaults.eq_shape_attrs(&other.defaults) {
            return false;
        }
        let mut a = self
            .spans_iter()
            .filter(|(_, attrs)| !attrs.eq_shape_attrs(&self.defaults));
        let mut b = other
            .spans_iter()
            .filter(|(_, attrs)| !attrs.eq_shape_attrs(&other.defaults));
        loop {
            match (a.next(), b.next()) {
                (None, None) => return true,
                (Some((ra, aa)), Some((rb, ab))) if ra == rb && aa.eq_shape_attrs(ab) => {}
                _ => return false,
            }
        }
    }

    /// Get the current attribute spans
    pub fn spans(&self) -> Vec<(&Range<usize>, &AttrsOwned)> {
        self.spans_iter().collect()
    }

    /// Get an iterator over the current attribute spans
    pub fn spans_iter(&self) -> impl Iterator<Item = (&Range<usize>, &AttrsOwned)> + '_ {
        self.spans.iter()
    }

    /// Clear the current attribute spans
    pub fn clear_spans(&mut self) {
        self.spans.clear();
    }

    /// Add an attribute span, removes any previous matching parts of spans
    pub fn add_span(&mut self, range: Range<usize>, attrs: &Attrs) {
        //do not support 1..1 or 2..1 even if by accident.
        if range.is_empty() {
            return;
        }

        self.spans.insert(range, AttrsOwned::new(attrs));
    }

    /// Get the attribute span for an index
    ///
    /// This returns a span that contains the index
    pub fn get_span(&self, index: usize) -> Attrs<'_> {
        self.spans
            .get(&index)
            .map(|v| v.as_attrs())
            .unwrap_or(self.defaults.as_attrs())
    }

    /// Split attributes list at an offset
    #[allow(clippy::missing_panics_doc)]
    pub fn split_off(&mut self, index: usize) -> Self {
        let mut new = Self::new(&self.defaults.as_attrs());
        let mut removes = Vec::new();

        //get the keys we need to remove or fix.
        for span in self.spans.iter() {
            if span.0.end <= index {
                continue;
            }

            if span.0.start >= index {
                removes.push((span.0.clone(), false));
            } else {
                removes.push((span.0.clone(), true));
            }
        }

        for (key, resize) in removes {
            let (range, attrs) = self
                .spans
                .get_key_value(&key.start)
                .map(|v| (v.0.clone(), v.1.clone()))
                .expect("attrs span not found");
            self.spans.remove(key);

            if resize {
                new.spans.insert(0..range.end - index, attrs.clone());
                self.spans.insert(range.start..index, attrs);
            } else {
                new.spans
                    .insert(range.start - index..range.end - index, attrs);
            }
        }
        new
    }

    /// Resets the attributes with new defaults.
    pub(crate) fn reset(mut self, default: &Attrs) -> Self {
        self.defaults = AttrsOwned::new(default);
        self.spans.clear();
        self
    }
}
