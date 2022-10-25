// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{Attrs, Font, FontMatches};

/// Access system fonts
pub struct FontSystem<'a> {
    pub locale: String,
    db: fontdb::Database,
    pub font_cache: Mutex<HashMap<fontdb::ID, Option<Arc<Font<'a>>>>>,
}

impl<'a> FontSystem<'a> {
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

        Self {
            locale,
            db,
            font_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_font(&'a self, id: fontdb::ID) -> Option<Arc<Font<'a>>> {
        let mut font_cache = self.font_cache.lock().unwrap();
        font_cache.entry(id).or_insert_with(|| {
            let face = self.db.face(id)?;
            match Font::new(face) {
                Some(font) => Some(Arc::new(font)),
                None => {
                    log::warn!("failed to load font '{}'", face.post_script_name);
                    None
                }
            }
        }).clone()
    }

    pub fn matches<F: Fn(&fontdb::FaceInfo) -> bool>(
        &'a self,
        f: F,
    ) -> FontMatches<'_> {
        let mut fonts = Vec::new();
        for face in self.db.faces() {
            if !f(face) {
                continue;
            }

            match self.get_font(face.id) {
                Some(font) => fonts.push(font),
                None => (),
            }
        }

        FontMatches {
            locale: &self.locale,
            fonts
        }
    }

    pub fn matches_attrs(&'a self, attrs: &Attrs) -> FontMatches<'_> {
        self.matches(|face| {
            let matched = attrs.matches(face);

            if matched {
                log::debug!(
                    "{:?}: family '{}' postscript name '{}' style {:?} weight {:?} stretch {:?} monospaced {:?}",
                    face.id,
                    face.family,
                    face.post_script_name,
                    face.style,
                    face.weight,
                    face.stretch,
                    face.monospaced
                );
            }

            matched
        })
    }
}
