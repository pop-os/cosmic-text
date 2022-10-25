// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Attrs, Font, FontMatches};

/// Access system fonts
pub struct FontSystem {
    pub locale: String,
    db: fontdb::Database,
}

impl FontSystem {
    pub fn new() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        });
        log::info!("Locale: {}", locale);

        let mut db = fontdb::Database::new();
        let now = std::time::Instant::now();
        db.load_system_fonts();
        //TODO: configurable default fonts
        db.set_monospace_family("Fira Mono");
        db.set_sans_serif_family("Fira Sans");
        db.set_serif_family("DejaVu Serif");
        log::info!(
            "Loaded {} font faces in {}ms.",
            db.len(),
            now.elapsed().as_millis()
        );

        //TODO only do this on demand!
        assert_eq!(db.len(), db.faces().len());
        for i in 0..db.len() {
            let id = db.faces()[i].id;
            unsafe { db.make_shared_face_data(id); }
        }

        Self { locale, db }
    }

    pub fn matches<F: Fn(&fontdb::FaceInfo) -> bool>(
        &self,
        f: F,
    ) -> Option<FontMatches<'_>> {
        let mut fonts = Vec::new();
        for face in self.db.faces() {
            if !f(face) {
                continue;
            }

            match Font::new(face) {
                Some(font) => fonts.push(font),
                None => {
                    log::warn!("failed to load font '{}'", face.post_script_name);
                }
            }
        }

        if !fonts.is_empty() {
            Some(FontMatches {
                locale: &self.locale,
                fonts
            })
        } else {
            None
        }
    }

    pub fn matches_attrs(&self, attrs: Attrs) -> Option<FontMatches<'_>> {
        self.matches(|info| {
            let matched = {
                info.style == attrs.style &&
                info.weight == attrs.weight &&
                info.stretch == attrs.stretch &&
                //TODO: smarter way of including emoji
                (info.monospaced == attrs.monospaced || info.post_script_name.contains("Emoji"))
            };

            if matched {
                log::debug!(
                    "{:?}: family '{}' postscript name '{}' style {:?} weight {:?} stretch {:?} monospaced {:?}",
                    info.id,
                    info.family,
                    info.post_script_name,
                    info.style,
                    info.weight,
                    info.stretch,
                    info.monospaced
                );
            }

            matched
        })
    }
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}
