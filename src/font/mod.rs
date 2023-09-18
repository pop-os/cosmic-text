// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

use core::fmt;

use alloc::sync::Arc;

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
    pub fn new(info: &fontdb::FaceInfo) -> Option<Self> {
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
