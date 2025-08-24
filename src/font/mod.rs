// SPDX-License-Identifier: MIT OR Apache-2.0

use harfrust::Shaper;
use skrifa::raw::ReadError;
use skrifa::{metrics::Metrics, prelude::*};
// re-export ttf_parser
pub use ttf_parser;
// re-export peniko::Font;
#[cfg(feature = "peniko")]
pub use peniko::Font as PenikoFont;

use core::fmt;

use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use self_cell::self_cell;

pub mod fallback;
pub use fallback::{Fallback, PlatformFallback};

pub use self::system::*;
mod system;

struct OwnedFaceData {
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    shaper_data: harfrust::ShaperData,
    shaper_instance: harfrust::ShaperInstance,
    metrics: Metrics,
}

self_cell!(
    struct OwnedFace {
        owner: OwnedFaceData,

        #[covariant]
        dependent: Shaper,
    }
);

struct FontMonospaceFallback {
    monospace_em_width: Option<f32>,
    scripts: Vec<[u8; 4]>,
    unicode_codepoints: Vec<u32>,
}

/// A font
pub struct Font {
    #[cfg(feature = "swash")]
    swash: (u32, swash::CacheKey),
    harfrust: OwnedFace,
    #[cfg(not(feature = "peniko"))]
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    #[cfg(feature = "peniko")]
    data: peniko::Font,
    id: fontdb::ID,
    monospace_fallback: Option<FontMonospaceFallback>,
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Font {
    pub const fn id(&self) -> fontdb::ID {
        self.id
    }

    pub fn monospace_em_width(&self) -> Option<f32> {
        self.monospace_fallback
            .as_ref()
            .and_then(|x| x.monospace_em_width)
    }

    pub fn scripts(&self) -> &[[u8; 4]] {
        self.monospace_fallback.as_ref().map_or(&[], |x| &x.scripts)
    }

    pub fn unicode_codepoints(&self) -> &[u32] {
        self.monospace_fallback
            .as_ref()
            .map_or(&[], |x| &x.unicode_codepoints)
    }

    pub fn data(&self) -> &[u8] {
        #[cfg(not(feature = "peniko"))]
        {
            (*self.data).as_ref()
        }
        #[cfg(feature = "peniko")]
        {
            self.data.data.data()
        }
    }

    pub fn shaper(&self) -> &harfrust::Shaper<'_> {
        &self.harfrust.borrow_dependent()
    }

    pub fn metrics(&self) -> &Metrics {
        &self.harfrust.borrow_owner().metrics
    }

    #[cfg(feature = "peniko")]
    pub fn as_peniko(&self) -> PenikoFont {
        self.data.clone()
    }

    #[cfg(feature = "swash")]
    pub fn as_swash(&self) -> swash::FontRef<'_> {
        let swash = &self.swash;
        swash::FontRef {
            data: self.data(),
            offset: swash.0,
            key: swash.1,
        }
    }
}

impl Font {
    pub fn new(db: &fontdb::Database, id: fontdb::ID, weight: fontdb::Weight) -> Option<Self> {
        let info = db.face(id)?;

        let monospace_fallback = if cfg!(feature = "monospace_fallback") {
            db.with_face_data(id, |font_data, face_index| {
                let face = ttf_parser::Face::parse(font_data, face_index).ok()?;
                let monospace_em_width = info
                    .monospaced
                    .then(|| {
                        let hor_advance = face.glyph_hor_advance(face.glyph_index(' ')?)?;
                        let upem = face.units_per_em();
                        Some(f32::from(hor_advance) / f32::from(upem))
                    })
                    .flatten();

                if info.monospaced && monospace_em_width.is_none() {
                    None?;
                }

                let scripts = face
                    .tables()
                    .gpos
                    .into_iter()
                    .chain(face.tables().gsub)
                    .flat_map(|table| table.scripts)
                    .map(|script| script.tag.to_bytes())
                    .collect();

                let mut unicode_codepoints = Vec::new();

                face.tables()
                    .cmap?
                    .subtables
                    .into_iter()
                    .filter(ttf_parser::cmap::Subtable::is_unicode)
                    .for_each(|subtable| {
                        unicode_codepoints.reserve(1024);
                        subtable.codepoints(|code_point| {
                            if subtable.glyph_index(code_point).is_some() {
                                unicode_codepoints.push(code_point);
                            }
                        });
                    });

                unicode_codepoints.shrink_to_fit();

                Some(FontMonospaceFallback {
                    monospace_em_width,
                    scripts,
                    unicode_codepoints,
                })
            })?
        } else {
            None
        };

        let data = match &info.source {
            fontdb::Source::Binary(data) => Arc::clone(data),
            #[cfg(feature = "std")]
            fontdb::Source::File(path) => {
                log::warn!("Unsupported fontdb Source::File('{}')", path.display());
                return None;
            }
            #[cfg(feature = "std")]
            fontdb::Source::SharedFile(_path, data) => Arc::clone(data),
        };

        // It's a bit unfortunate but we need to parse the data into a `FontRef`
        // twice--once to construct the HarfRust `ShaperInstance` and
        // `ShaperData`, and once to create the persistent `FontRef` tied to the
        // lifetime of the face data.
        let font_ref = FontRef::from_index((*data).as_ref(), info.index).ok()?;
        let location = font_ref
            .axes()
            .location([(Tag::new(b"wght"), weight.0 as f32)]);
        let metrics = font_ref.metrics(Size::unscaled(), &location);
        let (shaper_instance, shaper_data) = {
            (
                harfrust::ShaperInstance::from_coords(&font_ref, location.coords().iter().copied()),
                harfrust::ShaperData::new(&font_ref),
            )
        };

        Some(Self {
            id: info.id,
            monospace_fallback,
            #[cfg(feature = "swash")]
            swash: {
                let swash = swash::FontRef::from_index((*data).as_ref(), info.index as usize)?;
                (swash.offset, swash.key)
            },
            harfrust: OwnedFace::try_new(
                OwnedFaceData {
                    data: Arc::clone(&data),
                    shaper_data,
                    shaper_instance,
                    metrics,
                },
                |OwnedFaceData {
                     data,
                     shaper_data,
                     shaper_instance,
                     ..
                 }| {
                    let font_ref = FontRef::from_index((**data).as_ref(), info.index)?;
                    let shaper = shaper_data
                        .shaper(&font_ref)
                        .instance(Some(&shaper_instance))
                        .build();
                    Ok::<_, ReadError>(shaper)
                },
            )
            .ok()?,
            #[cfg(not(feature = "peniko"))]
            data,
            #[cfg(feature = "peniko")]
            data: peniko::Font::new(peniko::Blob::new(data), info.index),
        })
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_fonts_load_time() {
        use crate::FontSystem;
        use sys_locale::get_locale;

        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::Instant::now();

        let mut db = fontdb::Database::new();
        let locale = get_locale().expect("Local available");
        db.load_system_fonts();
        FontSystem::new_with_locale_and_db(locale, db);

        #[cfg(not(target_arch = "wasm32"))]
        println!("Fonts load time {}ms.", now.elapsed().as_millis());
    }
}
