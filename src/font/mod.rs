// SPDX-License-Identifier: MIT OR Apache-2.0

use core::ops::Deref;

pub(crate) mod fallback;

pub use self::system::*;
mod system;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(feature = "swash"), repr(transparent))]
/// Identifies a [`Font`] in a [`FontSystem`]
pub struct FontKey {
    pub id: fontdb::ID,
    #[cfg(feature = "swash")]
    pub swash: (u32, swash::CacheKey),
}

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

    pub fn from_key(db: &'a fontdb::Database, key: FontKey) -> Option<Self> {
        let info = db.face(key.id)?;
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
            swash: key.swash,
        })
    }

    pub fn key(&self) -> FontKey {
        FontKey {
            id: self.info.id,
            #[cfg(feature = "swash")]
            swash: self.swash,
        }
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
