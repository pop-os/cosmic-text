// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

use core::fmt;

use alloc::sync::Arc;

pub use self::system::*;
mod system;

mod owned_face {
    impl_self_ref!(OwnedFace, rustybuzz::Face<'static>, rustybuzz::Face<'this>);
}
use owned_face::*;

pub struct Font {
    #[cfg(feature = "swash")]
    swash: (u32, swash::CacheKey),
    rustybuzz: OwnedFace<Arc<dyn AsRef<[u8]> + Send + Sync>>,
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

    pub fn rustybuzz(&self) -> &rustybuzz::Face<'_> {
        self.rustybuzz.as_ref()
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
        #[allow(unused_variables)]
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
            rustybuzz: OwnedFace::new(data.clone(), |data| {
                rustybuzz::Face::from_slice(data.as_ref().as_ref(), info.index)
            })?,
            data,
        })
    }
}
