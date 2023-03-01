// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{Attrs, Font, FontKey};

/// Access system fonts
pub struct FontSystem {
    locale: String,
    db: fontdb::Database,
}

impl FontSystem {
    pub fn new() -> Self {
        let locale = "en-US".to_string();

        let mut db = fontdb::Database::new();
        {
            db.set_monospace_family("Fira Mono");
            db.set_sans_serif_family("Fira Sans");
            db.set_serif_family("DejaVu Serif");
        }

        Self { locale, db }
    }

    pub fn new_with_locale_and_db(locale: &str, db: fontdb::Database) -> Self {
        Self {
            locale: locale.to_string(),
            db,
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn db(&self) -> &fontdb::Database {
        &self.db
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

    pub fn get_font_key(&mut self, id: fontdb::ID) -> Option<FontKey> {
        Some(Font::new(self.db.face(id)?)?.key())
    }

    pub fn get_font_matches(&mut self, attrs: Attrs) -> Arc<Vec<FontKey>> {
        let mut font_keys = Vec::new();
        for face in self.db.faces() {
            if !attrs.matches(face) {
                continue;
            }

            let font_key = match self.get_font_key(face.id) {
                Some(font_key) => font_key,
                None => continue,
            };

            if self.get_font(font_key).is_some() {
                font_keys.push(font_key);
            }
        }

        Arc::new(font_keys)
    }
}
