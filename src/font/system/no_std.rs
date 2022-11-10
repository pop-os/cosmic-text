// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{Attrs, Font, FontMatches};

/// Access system fonts
pub struct FontSystem{
    locale: String,
    db: fontdb::Database,
}

impl FontSystem {
    pub fn new() -> Self {
        //TODO: get locale from argument?
        let locale = "en-US".to_string();

        //TODO: allow loading fonts from memory
        let mut db = fontdb::Database::new();
        {
            //TODO: configurable default fonts
            db.set_monospace_family("Fira Mono");
            db.set_sans_serif_family("Fira Sans");
            db.set_serif_family("DejaVu Serif");
        }

        Self {
            locale,
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
    pub fn get_font<'a>(&'a self, id: fontdb::ID) -> Option<Arc<Font<'a>>> {
        let face = self.db.face(id)?;
        match Font::new(face) {
            Some(font) => Some(Arc::new(font)),
            None => {
                log::warn!("failed to load font '{}'", face.post_script_name);
                None
            }
        }
    }

    pub fn get_font_matches<'a>(&'a self, attrs: Attrs) -> Arc<FontMatches<'a>> {
        let mut fonts = Vec::new();
        for face in self.db.faces() {
            if !attrs.matches(face) {
                continue;
            }

            if let Some(font) = self.get_font(face.id) {
                fonts.push(font);
            }
        }

        Arc::new(FontMatches {
            locale: &self.locale,
            default_family: self.db.family_name(&attrs.family).to_string(),
            fonts
        })
    }
}
