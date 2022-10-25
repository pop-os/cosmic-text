// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ops::Deref;

pub struct Font<'a> {
    pub info: &'a fontdb::FaceInfo,
    pub data: &'a [u8],
    pub index: u32,
    pub rustybuzz: rustybuzz::Face<'a>,
    #[cfg(feature = "swash")]
    pub swash: swash::FontRef<'a>,
}

impl<'a> Font<'a> {
    pub fn new(info: &'a fontdb::FaceInfo) -> Option<Self> {
        let data = match &info.source {
            fontdb::Source::Binary(data) => data.deref().as_ref(),
            fontdb::Source::File(path) => {
                log::warn!("Unsupported fontdb Source::File('{}')", path.display());
                return None;
            }
            fontdb::Source::SharedFile(_path, data) => data.deref().as_ref(),
        };

        Some(Self {
            info,
            data,
            index: info.index,
            rustybuzz: rustybuzz::Face::from_slice(data, info.index)?,
            #[cfg(feature = "swash")]
            swash: swash::FontRef::from_index(data, info.index as usize)?,
        })
    }
}
