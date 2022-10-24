// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{collections::HashMap, sync::Mutex};

use super::{CacheKey, CacheItem};

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
