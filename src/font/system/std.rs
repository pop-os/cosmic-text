// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{Attrs, AttrsOwned, Font, FontMatches};

#[ouroboros::self_referencing]
struct FontSystemInner {
    locale: String,
    db: fontdb::Database,
    #[borrows(db)]
    #[not_covariant]
    font_cache: Mutex<HashMap<fontdb::ID, Option<Arc<Font<'this>>>>>,
    #[borrows(locale, db)]
    #[not_covariant]
    font_matches_cache: Mutex<HashMap<AttrsOwned, Arc<FontMatches<'this>>>>,
}

/// Access system fonts
pub struct FontSystem(FontSystemInner);

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
    pub fn new_with_locale_and_db(locale: String, mut db: fontdb::Database) -> Self {
        {
            #[cfg(not(target_arch = "wasm32"))]
            let now = std::time::Instant::now();

            //TODO only do this on demand!
            for id in db.faces().map(|face| face.id).collect::<Vec<_>>() {
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

        Self(
            FontSystemInnerBuilder {
                locale,
                db,
                font_cache_builder: |_| Mutex::new(HashMap::new()),
                font_matches_cache_builder: |_, _| Mutex::new(HashMap::new()),
            }
            .build(),
        )
    }

    pub fn locale(&self) -> &str {
        self.0.borrow_locale()
    }

    pub fn db(&self) -> &fontdb::Database {
        self.0.borrow_db()
    }

    pub fn into_locale_and_db(self) -> (String, fontdb::Database) {
        let heads = self.0.into_heads();
        (heads.locale, heads.db)
    }

    // Clippy false positive
    #[allow(clippy::needless_lifetimes)]
    pub fn get_font<'a>(&'a self, id: fontdb::ID) -> Option<Arc<Font<'a>>> {
        self.0.with(|fields| get_font(&fields, id))
    }

    pub fn get_font_matches<'a>(&'a self, attrs: Attrs) -> Arc<FontMatches<'a>> {
        self.0.with(|fields| {
            let mut font_matches_cache = fields
                .font_matches_cache
                .lock()
                .expect("failed to lock font matches cache");
            //TODO: do not create AttrsOwned unless entry does not already exist
            font_matches_cache
                .entry(AttrsOwned::new(attrs))
                .or_insert_with(|| {
                    #[cfg(not(target_arch = "wasm32"))]
                    let now = std::time::Instant::now();

                    let mut fonts = Vec::new();
                    for face in fields.db.faces() {
                        if !attrs.matches(face) {
                            continue;
                        }

                        if let Some(font) = get_font(&fields, face.id) {
                            fonts.push(font);
                        }
                    }

                    let font_matches = Arc::new(FontMatches {
                        locale: fields.locale,
                        default_family: fields.db.family_name(&attrs.family).to_string(),
                        fonts,
                    });

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let elapsed = now.elapsed();
                        log::debug!("font matches for {:?} in {:?}", attrs, elapsed);
                    }

                    font_matches
                })
                .clone()
        })
    }
}

fn get_font<'b>(
    fields: &ouroboros_impl_font_system_inner::BorrowedFields<'_, 'b>,
    id: fontdb::ID,
) -> Option<Arc<Font<'b>>> {
    fields
        .font_cache
        .lock()
        .expect("failed to lock font cache")
        .entry(id)
        .or_insert_with(|| {
            let face = fields.db.face(id)?;
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
