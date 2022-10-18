use std::{collections::HashMap, sync::Mutex};

pub mod fallback;

pub use self::cache::*;
mod cache;

pub use self::layout::*;
mod layout;

pub use self::matches::*;
mod matches;

pub use self::shape::*;
mod shape;

pub use self::system::*;
mod system;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FontCacheKey {
    glyph_id: u16,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct FontLineIndex(usize);

impl FontLineIndex {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

pub struct Font<'a> {
    pub info: &'a fontdb::FaceInfo,
    pub data: &'a [u8],
    pub index: u32,
    pub rustybuzz: rustybuzz::Face<'a>,
    pub swash: swash::FontRef<'a>,
    pub scale_context: Mutex<swash::scale::ScaleContext>,
    pub cache: Mutex<HashMap<CacheKey, CacheItem>>,
}

impl<'a> Font<'a> {
    pub fn new(info: &'a fontdb::FaceInfo, data: &'a [u8], index: u32) -> Option<Self> {
        Some(Self {
            info,
            data,
            index,
            rustybuzz: rustybuzz::Face::from_slice(data, index)?,
            swash: swash::FontRef::from_index(data, index as usize)?,
            scale_context: Mutex::new(swash::scale::ScaleContext::new()),
            cache: Mutex::new(HashMap::new()),
        })
    }
}
