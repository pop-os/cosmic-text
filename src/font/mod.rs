// SPDX-License-Identifier: MIT OR Apache-2.0

use core::ops::Deref;

pub(crate) mod fallback;

pub use self::matches::*;
mod matches;

pub use self::system::*;
mod system;

/// A font
pub struct Font<'a> {
    pub info: &'a fontdb::FaceInfo,
    pub data: &'a [u8],
    pub rustybuzz: rustybuzz::Face<'a>,
    #[cfg(feature = "swash")]
    pub swash: (u32, swash::CacheKey),
}

impl<'a> Font<'a> {
    pub fn new(info: &'a fontdb::FaceInfo) -> Option<Self> {
        let data = match &info.source {
            fontdb::Source::Binary(data) => data.deref().as_ref(),
            #[cfg(feature = "std")]
            fontdb::Source::File(path) => {
                log::warn!("Unsupported fontdb Source::File('{}')", path.display());
                return None;
            }
            #[cfg(feature = "std")]
            fontdb::Source::SharedFile(_path, data) => data.deref().as_ref(),
        };

        Some(Self {
            info,
            data,
            rustybuzz: rustybuzz::Face::from_slice(data, info.index)?,
            #[cfg(feature = "swash")]
            swash: {
                let swash = swash::FontRef::from_index(data, info.index as usize)?;
                (swash.offset, swash.key)
            },
        })
    }

    pub fn name(&self) -> &str {
        if let Some((name, _)) = self.info.families.first() {
            name
        } else {
            &self.info.post_script_name
        }
    }

    pub fn contains_family(&self, family: &str) -> bool {
        self.info.families.iter().any(|(name, _)| name == family)
    }

    #[cfg(feature = "swash")]
    pub fn as_swash(&self) -> swash::FontRef {
        swash::FontRef {
            data: self.data,
            offset: self.swash.0,
            key: self.swash.1,
        }
    }
}
