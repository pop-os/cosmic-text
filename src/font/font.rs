// SPDX-License-Identifier: MIT OR Apache-2.0

pub struct Font<'a> {
    pub info: &'a fontdb::FaceInfo,
    pub data: &'a [u8],
    pub index: u32,
    pub rustybuzz: rustybuzz::Face<'a>,
    #[cfg(feature = "swash")]
    pub swash: swash::FontRef<'a>,
}

impl<'a> Font<'a> {
    pub fn new(info: &'a fontdb::FaceInfo, data: &'a [u8], index: u32) -> Option<Self> {
        Some(Self {
            info,
            data,
            index,
            rustybuzz: rustybuzz::Face::from_slice(data, index)?,
            #[cfg(feature = "swash")]
            swash: swash::FontRef::from_index(data, index as usize)?,
        })
    }
}
