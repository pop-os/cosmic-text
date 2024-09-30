// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

// re-export ttf_parser
pub use ttf_parser;

use core::fmt;

use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use rustybuzz::Face as RustybuzzFace;
use self_cell::self_cell;

pub use self::system::*;
mod system;

self_cell!(
    struct OwnedFace {
        owner: Arc<dyn AsRef<[u8]> + Send + Sync>,

        #[covariant]
        dependent: RustybuzzFace,
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
    rustybuzz: OwnedFace,
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
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
    pub fn id(&self) -> fontdb::ID {
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
        (*self.data).as_ref()
    }

    pub fn rustybuzz(&self) -> &RustybuzzFace<'_> {
        self.rustybuzz.borrow_dependent()
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
    #[cfg(feature = "monospace_fallback")]
    fn proportional_monospaced(face: &ttf_parser::Face) -> Option<bool> {
        use ttf_parser::cmap::{Format, Subtable};
        use ttf_parser::Face;

        // Pick a unicode cmap subtable to check against its glyphs
        let cmap = face.tables().cmap.as_ref()?;
        let subtable12 = cmap.subtables.into_iter().find(|subtable| {
            subtable.is_unicode() && matches!(subtable.format, Format::SegmentedCoverage(_))
        });
        let subtable4_fn = || {
            cmap.subtables.into_iter().find(|subtable| {
                subtable.is_unicode()
                    && matches!(subtable.format, Format::SegmentMappingToDeltaValues(_))
            })
        };
        let unicode_subtable = subtable12.or_else(subtable4_fn)?;

        fn is_proportional(
            face: &Face,
            unicode_subtable: Subtable,
            code_point_iter: impl Iterator<Item = u32>,
        ) -> Option<bool> {
            // Fonts like "Noto Sans Mono" have single, double, AND triple width glyphs.
            // So we check proportionality up to 3x width, and assume non-proportionality
            // once a forth non-zero advance value is encountered.
            const MAX_ADVANCES: usize = 3;

            let mut advances = Vec::with_capacity(MAX_ADVANCES);

            for code_point in code_point_iter {
                if let Some(glyph_id) = unicode_subtable.glyph_index(code_point) {
                    match face.glyph_hor_advance(glyph_id) {
                        Some(advance) if advance != 0 => match advances.binary_search(&advance) {
                            Err(_) if advances.len() == MAX_ADVANCES => return Some(false),
                            Err(i) => advances.insert(i, advance),
                            Ok(_) => (),
                        },
                        _ => (),
                    }
                }
            }

            let mut advances = advances.into_iter();
            let smallest = advances.next()?;
            Some(advances.find(|advance| advance % smallest > 0).is_none())
        }

        match unicode_subtable.format {
            Format::SegmentedCoverage(subtable12) => {
                is_proportional(face, unicode_subtable, subtable12.codepoints_iter())
            }
            Format::SegmentMappingToDeltaValues(subtable4) => {
                is_proportional(face, unicode_subtable, subtable4.codepoints_iter())
            }
            _ => unreachable!(),
        }
    }

    pub fn new(db: &fontdb::Database, id: fontdb::ID, is_monospace: bool) -> Option<Self> {
        let info = db.face(id)?;

        let monospace_fallback = if cfg!(feature = "monospace_fallback") && is_monospace {
            db.with_face_data(id, |font_data, face_index| {
                let face = ttf_parser::Face::parse(font_data, face_index).ok()?;
                let monospace_em_width = {
                    || {
                        let hor_advance = face.glyph_hor_advance(face.glyph_index(' ')?)? as f32;
                        let upem = face.units_per_em() as f32;
                        Some(hor_advance / upem)
                    }
                }();

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
                    .filter(|subtable| subtable.is_unicode())
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

        Some(Self {
            id: info.id,
            monospace_fallback,
            #[cfg(feature = "swash")]
            swash: {
                let swash = swash::FontRef::from_index((*data).as_ref(), info.index as usize)?;
                (swash.offset, swash.key)
            },
            rustybuzz: OwnedFace::try_new(Arc::clone(&data), |data| {
                RustybuzzFace::from_slice((**data).as_ref(), info.index).ok_or(())
            })
            .ok()?,
            data,
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
        let locale = get_locale().unwrap();
        db.load_system_fonts();
        FontSystem::new_with_locale_and_db(locale, db);

        #[cfg(not(target_arch = "wasm32"))]
        println!("Fonts load time {}ms.", now.elapsed().as_millis())
    }
}
