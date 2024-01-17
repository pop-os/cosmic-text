// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

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

/// A font
pub struct Font {
    #[cfg(feature = "swash")]
    swash: (u32, swash::CacheKey),
    rustybuzz: OwnedFace,
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    id: fontdb::ID,
    monospace_em_width: Option<f32>,
    scripts: Vec<[u8; 4]>,
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
        self.monospace_em_width
    }

    pub fn scripts(&self) -> &[[u8; 4]] {
        &self.scripts
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
    pub fn new(db: &fontdb::Database, id: fontdb::ID) -> Option<Self> {
        let info = db.face(id)?;

        let (monospace_em_width, scripts) = {
            db.with_face_data(id, |font_data, face_index| {
                let face = ttf_parser::Face::parse(font_data, face_index).ok()?;
                let monospace_em_width = info
                    .monospaced
                    .then(|| {
                        let hor_advance = face.glyph_hor_advance(face.glyph_index(' ')?)? as f32;
                        let upem = face.units_per_em() as f32;
                        Some(hor_advance / upem)
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
                    .map(|table| table.scripts)
                    .flatten()
                    .map(|script| script.tag.to_bytes())
                    .collect();
                Some((monospace_em_width, scripts))
            })?
        }?;

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
            monospace_em_width,
            scripts,
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
