// SPDX-License-Identifier: MIT OR Apache-2.0
pub(crate) mod fallback;

use fontdb::FaceInfo;

pub use self::system::*;
mod system;

/// A font
pub struct Font(FontInner);

#[ouroboros::self_referencing]
#[allow(dead_code)]
struct FontInner {
    info: fontdb::FaceInfo,
    #[borrows(info)]
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
            fontdb::Source::Binary(data) => (**data).as_ref(),
            #[cfg(feature = "std")]
            fontdb::Source::File(path) => {
                log::warn!("Unsupported fontdb Source::File('{}')", path.display());
                return None;
            }
            #[cfg(feature = "std")]
            fontdb::Source::SharedFile(_path, data) => (**data).as_ref(),
        };
        Some(Self(
            FontInnerTryBuilder {
                info: info.clone(),
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
                rustybuzz_builder: |info| {
                    rustybuzz::Face::from_slice(get_data(info), info.index).ok_or(())
                },
            }
            .try_build()
            .ok()?,
        ))
    }

    pub fn info(&self) -> &FaceInfo {
        self.0.borrow_info()
    }

    pub fn data(&self) -> &[u8] {
        get_data(self.0.borrow_info())
    }

    pub fn rustybuzz(&self) -> &rustybuzz::Face {
        self.0.borrow_rustybuzz()
    }

    pub fn name(&self) -> &str {
        if let Some((name, _)) = self.info().families.first() {
            name
        } else {
            &self.info().post_script_name
        }
    }

    pub fn contains_family(&self, family: &str) -> bool {
        self.info().families.iter().any(|(name, _)| name == family)
    }

    #[cfg(feature = "swash")]
    pub fn as_swash(&self) -> swash::FontRef {
        let info = self.0.borrow_info();
        let swash = self.0.borrow_swash();
        swash::FontRef {
            data: get_data(info),
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

fn get_data(info: &FaceInfo) -> &[u8] {
    match &info.source {
        fontdb::Source::Binary(data) => (**data).as_ref(),
        #[cfg(feature = "std")]
        fontdb::Source::File(path) => {
            // This should never happen, because `Font::new` verified the source isn't a file
            panic!("Unsupported fontdb Source::File('{}')", path.display());
        }
        #[cfg(feature = "std")]
        fontdb::Source::SharedFile(_path, data) => (**data).as_ref(),
    }
}
