// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{collections::HashMap, sync::Arc};

use crate::{Attrs, AttrsOwned, Font};

/// Access system fonts
pub struct FontSystem {
    locale: String,
    db: fontdb::Database,
    font_cache: HashMap<fontdb::ID, Option<Arc<Font>>>,
    font_matches_cache: HashMap<AttrsOwned, Arc<Vec<fontdb::ID>>>,
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

            #[cfg(target_os = "redox")]
            db.load_fonts_dir("/ui/fonts");

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
    pub fn new_with_locale_and_db(locale: String, db: fontdb::Database) -> Self {
        Self {
            locale,
            db,
            font_cache: HashMap::new(),
            font_matches_cache: HashMap::new(),
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn db(&self) -> &fontdb::Database {
        &self.db
    }

    pub fn db_mut(&mut self) -> &mut fontdb::Database {
        self.font_matches_cache.clear();
        &mut self.db
    }

    pub fn into_locale_and_db(self) -> (String, fontdb::Database) {
        (self.locale, self.db)
    }

    pub fn get_font(&mut self, id: fontdb::ID) -> Option<Arc<Font>> {
        get_font(&mut self.font_cache, &mut self.db, id)
    }

    pub fn get_font_matches(&mut self, attrs: Attrs) -> Arc<Vec<fontdb::ID>> {
        self.font_matches_cache
            //TODO: do not create AttrsOwned unless entry does not already exist
            .entry(AttrsOwned::new(attrs))
            .or_insert_with(|| {
                #[cfg(not(target_arch = "wasm32"))]
                let now = std::time::Instant::now();

                let ids = self
                    .db
                    .faces()
                    .filter(|face| attrs.matches(face))
                    .map(|face| face.id)
                    .collect::<Vec<_>>();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let elapsed = now.elapsed();
                    log::debug!("font matches for {:?} in {:?}", attrs, elapsed);
                }

                Arc::new(ids)
            })
            .clone()
    }

    pub fn discard_unused_fonts(&mut self) {
        self.font_cache.retain(|_, font| {
            if let Some(font) = font {
                if Arc::strong_count(font) == 1 {
                    self.db.make_face_data_unshared(font.id());
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });
    }
}

fn get_font(
    font_cache: &mut HashMap<fontdb::ID, Option<Arc<Font>>>,
    db: &mut fontdb::Database,
    id: fontdb::ID,
) -> Option<Arc<Font>> {
    font_cache
        .entry(id)
        .or_insert_with(|| {
            unsafe {
                db.make_shared_face_data(id);
            }
            let face = db.face(id)?;
            match Font::new(face) {
                Some(font) => Some(Arc::new(font)),
                None => {
                    log::warn!("failed to load font '{}'", face.post_script_name);
                    None
                }
            }
        })
        .clone()
}
