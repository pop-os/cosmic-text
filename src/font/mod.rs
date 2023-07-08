// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

use alloc::boxed::Box;
use alloc::sync::Arc;

pub use self::system::*;
mod system;

pub use font_inner::Font;

/// Encapsulates the self-referencing `Font` struct to ensure all field accesses have to go through
/// safe methods.
mod font_inner {
    use super::*;
    use aliasable::boxed::AliasableBox;
    use core::fmt;

    /// A font
    //
    // # Safety invariant
    //
    // `data` must never have a mutable reference taken, nor be modified during the lifetime of
    // this `Font`.
    pub struct Font {
        #[cfg(feature = "swash")]
        swash: (u32, swash::CacheKey),
        rustybuzz: rustybuzz::Face<'static>,
        // Note: This field must be after rustybuzz to ensure that it is dropped later. Otherwise
        // there would be a dangling reference when dropping rustybuzz.
        data: aliasable::boxed::AliasableBox<Arc<dyn AsRef<[u8]> + Send + Sync>>,
        id: fontdb::ID,
    }

    impl fmt::Debug for Font {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Font")
                .field("id", &self.id)
                .finish_non_exhaustive()
        }
    }

    pub(super) struct FontTryBuilder<
        RustybuzzBuilder: for<'this> FnOnce(
            &'this Arc<dyn AsRef<[u8]> + Send + Sync>,
        ) -> Option<rustybuzz::Face<'this>>,
    > {
        pub(super) id: fontdb::ID,
        pub(super) data: Arc<dyn AsRef<[u8]> + Send + Sync>,
        pub(super) rustybuzz_builder: RustybuzzBuilder,
        #[cfg(feature = "swash")]
        pub(super) swash: (u32, swash::CacheKey),
    }
    impl<
            RustybuzzBuilder: for<'this> FnOnce(
                &'this Arc<dyn AsRef<[u8]> + Send + Sync>,
            ) -> Option<rustybuzz::Face<'this>>,
        > FontTryBuilder<RustybuzzBuilder>
    {
        pub(super) fn try_build(self) -> Option<Font> {
            unsafe fn change_lifetime<'old, 'new: 'old, T: 'new>(data: &'old T) -> &'new T {
                &*(data as *const _)
            }

            let data: AliasableBox<Arc<dyn AsRef<[u8]> + Send + Sync>> =
                AliasableBox::from_unique(Box::new(self.data));

            // Safety: We use AliasableBox to allow the references in rustybuzz::Face to alias with
            // the data stored behind the AliasableBox. In addition the entire public interface of
            // Font ensures that no mutable reference is given to data. And finally we use
            // for<'this> for the rustybuzz_builder to ensure it can't leak a reference. Combined
            // this ensures that it is sound to produce a self-referential type.
            let rustybuzz = (self.rustybuzz_builder)(unsafe { change_lifetime(&*data) })?;

            Some(Font {
                id: self.id,
                data,
                rustybuzz,
                #[cfg(feature = "swash")]
                swash: self.swash,
            })
        }
    }

    impl Font {
        pub fn id(&self) -> fontdb::ID {
            self.id
        }

        pub fn data(&self) -> &[u8] {
            // Safety: This only gives an immutable access to `data`.
            (**self.data).as_ref()
        }

        pub fn rustybuzz(&self) -> &rustybuzz::Face<'_> {
            &self.rustybuzz
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
        font_inner::FontTryBuilder {
            id: info.id,
            #[cfg(feature = "swash")]
            swash: {
                let swash = swash::FontRef::from_index((*data).as_ref(), info.index as usize)?;
                (swash.offset, swash.key)
            },
            data,
            rustybuzz_builder: |data| rustybuzz::Face::from_slice((**data).as_ref(), info.index),
        }
        .try_build()
    }
}
