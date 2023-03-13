// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

use alloc::sync::Arc;

pub use self::system::*;
mod system;

/// A font
pub struct Font(FontInner);

#[ouroboros::self_referencing]
#[allow(dead_code)]
struct FontInner {
    id: fontdb::ID,
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    #[borrows(data)]
    #[covariant]
    rustybuzz: rustybuzz::Face<'this>,
    // workaround, since ouroboros does not work with #[cfg(feature = "swash")]
    swash: SwashKey,
}

#[cfg(feature = "swash")]
pub type SwashKey = (u32, swash::CacheKey);

#[cfg(not(feature = "swash"))]
pub type SwashKey = ();

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
        Some(Self(
            FontInnerTryBuilder {
                id: info.id,
                swash: {
                    #[cfg(feature = "swash")]
                    let swash = {
                        let swash =
                            swash::FontRef::from_index((*data).as_ref(), info.index as usize)?;
                        (swash.offset, swash.key)
                    };
                    #[cfg(not(feature = "swash"))]
                    let swash = ();
                    swash
                },
                data,
                rustybuzz_builder: |data| {
                    rustybuzz::Face::from_slice((**data).as_ref(), info.index).ok_or(())
                },
            }
            .try_build()
            .ok()?,
        ))
    }

    pub fn id(&self) -> fontdb::ID {
        *self.0.borrow_id()
    }

    pub fn data(&self) -> &[u8] {
        (**self.0.borrow_data()).as_ref()
    }

    pub fn rustybuzz(&self) -> &rustybuzz::Face {
        self.0.borrow_rustybuzz()
    }

    #[cfg(feature = "swash")]
    pub fn as_swash(&self) -> swash::FontRef {
        let swash = self.0.borrow_swash();
        swash::FontRef {
            data: self.data(),
            offset: swash.0,
            key: swash.1,
        }
    }

    // This is used to prevent warnings due to the swash field being unused.
    #[cfg(not(feature = "swash"))]
    #[allow(dead_code)]
    fn as_swash(&self) {
        self.0.borrow_swash();
    }
}
