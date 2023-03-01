// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "swash")]
use std::collections::hash_map::Entry;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{Attrs, AttrsOwned, Font, FontKey};

/// Access system fonts
pub struct FontSystem {
    locale: String,
    db: fontdb::Database,
    font_matches_cache: Mutex<HashMap<AttrsOwned, Arc<Vec<FontKey>>>>,
    #[cfg(feature = "swash")]
    font_key_cache: Mutex<HashMap<fontdb::ID, Option<FontKey>>>,
}

impl FontSystem {
    /// Create a new [`FontSystem`], that allows access to any installed system fonts
    ///
    /// # Timing
    ///
    /// This function takes some time to run. On the release build, it can take up to a second,
    /// while debug builds can take up to ten times longer. For this reason, it should only be
    /// called once, and the resulting [`FontSystem`] should be shared.
    pub fn new() -> Self {
        Self::new_with_fonts(std::iter::empty())
    }

    pub fn new_with_fonts(fonts: impl Iterator<Item = fontdb::Source>) -> Self {
        let locale = sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        });
        log::debug!("Locale: {}", locale);

        let mut db = fontdb::Database::new();
        {
            #[cfg(not(target_arch = "wasm32"))]
            let now = std::time::Instant::now();

            db.load_system_fonts();

            for source in fonts {
                db.load_font_source(source);
            }

            //TODO: configurable default fonts
            db.set_monospace_family("Fira Mono");
            db.set_sans_serif_family("Fira Sans");
            db.set_serif_family("DejaVu Serif");

            #[cfg(not(target_arch = "wasm32"))]
            log::info!(
                "Parsed {} font faces in {}ms.",
                db.len(),
                now.elapsed().as_millis()
            );
        }

        Self::new_with_locale_and_db(locale, db)
    }

    /// Create a new [`FontSystem`], manually specifying the current locale and font database.
    pub fn new_with_locale_and_db(locale: String, mut db: fontdb::Database) -> Self {
        {
            #[cfg(not(target_arch = "wasm32"))]
            let now = std::time::Instant::now();

            //TODO only do this on demand!
            for i in 0..db.faces().len() {
                let id = db.faces()[i].id;
                unsafe {
                    db.make_shared_face_data(id);
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            log::info!(
                "Mapped {} font faces in {}ms.",
                db.len(),
                now.elapsed().as_millis()
            );
        }

        Self {
            locale,
            db,
            font_matches_cache: Mutex::new(HashMap::new()),
            #[cfg(feature = "swash")]
            font_key_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn db(&self) -> &fontdb::Database {
        &self.db
    }

    pub fn into_locale_and_db(self) -> (String, fontdb::Database) {
        (self.locale, self.db)
    }

    // Clippy false positive
    #[allow(clippy::needless_lifetimes)]
    pub fn get_font<'a>(&'a self, key: FontKey) -> Option<Font<'a>> {
        match Font::from_key(&self.db, key) {
            Some(font) => Some(font),
            None => {
                let face = self.db.face(key.id)?;
                log::warn!("failed to load font '{}'", face.post_script_name);
                None
            }
        }
    }

    #[cfg(feature = "swash")]
    pub fn get_font_key(&self, id: fontdb::ID) -> Option<FontKey> {
        let mut font_key_cache = self
            .font_key_cache
            .lock()
            .expect("failed to lock font matches cache");
        match font_key_cache.entry(id) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let key = self.db.face(id).and_then(Font::new).as_ref().map(Font::key);
                entry.insert(key);
                key
            }
        }
    }

    #[cfg(not(feature = "swash"))]
    pub fn get_font_key(&self, id: fontdb::ID) -> Option<FontKey> {
        Some(Font::new(self.db.face(id)?)?.key())
    }

    pub fn get_font_matches(&self, attrs: Attrs) -> Arc<Vec<FontKey>> {
        let mut font_matches_cache = self
            .font_matches_cache
            .lock()
            .expect("failed to lock font matches cache");
        //TODO: do not create AttrsOwned unless entry does not already exist
        font_matches_cache
            .entry(AttrsOwned::new(attrs))
            .or_insert_with(|| {
                #[cfg(not(target_arch = "wasm32"))]
                let now = std::time::Instant::now();

                let mut font_keys = Vec::new();
                for face in self.db.faces() {
                    if !attrs.matches(face) {
                        continue;
                    }

                    if let Some(key) = self.get_font_key(face.id) {
                        font_keys.push(key);
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let elapsed = now.elapsed();
                    log::debug!("font matches for {:?} in {:?}", attrs, elapsed);
                }

                Arc::new(font_keys)
            })
            .clone()
    }
}
