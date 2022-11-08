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
    pub fn new() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        });
        log::info!("Locale: {}", locale);

        let mut db = fontdb::Database::new();
        {
            let now = std::time::Instant::now();

            db.load_system_fonts();
            //TODO: configurable default fonts
            db.set_monospace_family("Fira Mono");
            db.set_sans_serif_family("Fira Sans");
            db.set_serif_family("DejaVu Serif");

            log::info!(
                "Parsed {} font faces in {}ms.",
                db.len(),
                now.elapsed().as_millis()
            );
        }

        {
            let now = std::time::Instant::now();

            //TODO only do this on demand!
            assert_eq!(db.len(), db.faces().len());
            for i in 0..db.len() {
                let id = db.faces()[i].id;
                unsafe { db.make_shared_face_data(id); }
            }

            log::info!(
                "Mapped {} font faces in {}ms.",
                db.len(),
                now.elapsed().as_millis()
            );
        }

        Self(FontSystemInnerBuilder {
            locale,
            db,
            font_cache_builder: |_| Mutex::new(HashMap::new()),
            font_matches_cache_builder: |_, _| Mutex::new(HashMap::new())
        }.build())
    }

    pub fn locale(&self) -> &str {
        self.0.borrow_locale()
    }

    pub fn db(&self) -> &fontdb::Database {
        self.0.borrow_db()
    }

    pub fn get_font<'a>(&'a self, id: fontdb::ID) -> Option<Arc<Font<'a>>> {
        self.0.with(|fields| {
            get_font(&fields, id)
        })
    }

    pub fn get_font_matches<'a>(&'a self, attrs: Attrs) -> Arc<FontMatches<'a>> {
        self.0.with(|fields| {
            let mut font_matches_cache = fields.font_matches_cache.lock().unwrap();
            //TODO: do not create AttrsOwned unless entry does not already exist
            font_matches_cache.entry(AttrsOwned::new(attrs)).or_insert_with(|| {
                let now = std::time::Instant::now();

                let mut fonts = Vec::new();
                for face in fields.db.faces() {
                    if !attrs.matches(face) {
                        continue;
                    }

                    match get_font(&fields, face.id) {
                        Some(font) => fonts.push(font),
                        None => (),
                    }
                }

                let font_matches = Arc::new(FontMatches {
                    locale: fields.locale,
                    default_family: fields.db.family_name(&attrs.family).to_string(),
                    fonts
                });

                let elapsed = now.elapsed();
                log::debug!("font matches for {:?} in {:?}", attrs, elapsed);

                font_matches
            }).clone()
        })
    }
}

fn get_font<'b>(fields: &ouroboros_impl_font_system_inner::BorrowedFields<'_, 'b>, id: fontdb::ID) -> Option<Arc<Font<'b>>> {
    fields.font_cache.lock().unwrap().entry(id).or_insert_with(|| {
        let face = fields.db.face(id)?;
        match Font::new(face) {
            Some(font) => Some(Arc::new(font)),
            None => {
                log::warn!("failed to load font '{}'", face.post_script_name);
                None
            }
        }
    }).clone()
}
