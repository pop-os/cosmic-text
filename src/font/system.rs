use crate::{Attrs, AttrsOwned, Font};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::ops::{Deref, DerefMut};

type BuildHasher = core::hash::BuildHasherDefault<rustc_hash::FxHasher>;

#[cfg(feature = "std")]
type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasher>;
#[cfg(not(feature = "std"))]
type HashMap<K, V> = hashbrown::HashMap<K, V, BuildHasher>;

// re-export fontdb and rustybuzz
pub use fontdb;
pub use rustybuzz;

/// Access to the system fonts.
pub struct FontSystem {
    /// The locale of the system.
    locale: String,

    /// The underlying font database.
    db: fontdb::Database,

    /// Cache for loaded fonts from the database.
    font_cache: HashMap<fontdb::ID, Option<Arc<Font>>>,

    /// Cache for font matches.
    font_matches_cache: HashMap<AttrsOwned, Arc<Vec<fontdb::ID>>>,
}

impl fmt::Debug for FontSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontSystem")
            .field("locale", &self.locale)
            .field("db", &self.db)
            .finish()
    }
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
        Self::new_with_fonts(core::iter::empty())
    }

    /// Create a new [`FontSystem`] with a pre-specified set of fonts.
    pub fn new_with_fonts(fonts: impl IntoIterator<Item = fontdb::Source>) -> Self {
        let locale = Self::get_locale();
        log::debug!("Locale: {}", locale);

        let mut db = fontdb::Database::new();

        //TODO: configurable default fonts
        db.set_monospace_family("Fira Mono");
        db.set_sans_serif_family("Fira Sans");
        db.set_serif_family("DejaVu Serif");

        Self::load_fonts(&mut db, fonts.into_iter());

        Self::new_with_locale_and_db(locale, db)
    }

    /// Create a new [`FontSystem`] with a pre-specified locale and font database.
    pub fn new_with_locale_and_db(locale: String, db: fontdb::Database) -> Self {
        Self {
            locale,
            db,
            font_cache: HashMap::default(),
            font_matches_cache: HashMap::default(),
        }
    }

    /// Get the locale.
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Get the database.
    pub fn db(&self) -> &fontdb::Database {
        &self.db
    }

    /// Get a mutable reference to the database.
    pub fn db_mut(&mut self) -> &mut fontdb::Database {
        self.font_matches_cache.clear();
        &mut self.db
    }

    /// Consume this [`FontSystem`] and return the locale and database.
    pub fn into_locale_and_db(self) -> (String, fontdb::Database) {
        (self.locale, self.db)
    }

    /// Get a font by its ID.
    pub fn get_font(&mut self, id: fontdb::ID) -> Option<Arc<Font>> {
        self.font_cache
            .entry(id)
            .or_insert_with(|| {
                #[cfg(feature = "std")]
                unsafe {
                    self.db.make_shared_face_data(id);
                }
                let face = self.db.face(id)?;
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

    pub fn get_font_matches(&mut self, attrs: Attrs<'_>) -> Arc<Vec<fontdb::ID>> {
        self.font_matches_cache
            //TODO: do not create AttrsOwned unless entry does not already exist
            .entry(AttrsOwned::new(attrs))
            .or_insert_with(|| {
                #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
                let now = std::time::Instant::now();

                let ids = self
                    .db
                    .faces()
                    .filter(|face| attrs.matches(face))
                    .map(|face| face.id)
                    .collect::<Vec<_>>();

                #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
                {
                    let elapsed = now.elapsed();
                    log::debug!("font matches for {:?} in {:?}", attrs, elapsed);
                }

                Arc::new(ids)
            })
            .clone()
    }

    #[cfg(feature = "std")]
    fn get_locale() -> String {
        sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        })
    }

    #[cfg(not(feature = "std"))]
    fn get_locale() -> String {
        String::from("en-US")
    }

    #[cfg(feature = "std")]
    fn load_fonts(db: &mut fontdb::Database, fonts: impl Iterator<Item = fontdb::Source>) {
        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::Instant::now();

        #[cfg(target_os = "redox")]
        db.load_fonts_dir("/ui/fonts");

        db.load_system_fonts();

        for source in fonts {
            db.load_font_source(source);
        }

        #[cfg(not(target_arch = "wasm32"))]
        log::info!(
            "Parsed {} font faces in {}ms.",
            db.len(),
            now.elapsed().as_millis()
        );
    }

    #[cfg(not(feature = "std"))]
    fn load_fonts(db: &mut fontdb::Database, fonts: impl Iterator<Item = fontdb::Source>) {
        for source in fonts {
            db.load_font_source(source);
        }
    }
}

/// A value borrowed together with an [`FontSystem`]
#[derive(Debug)]
pub struct BorrowedWithFontSystem<'a, T> {
    pub(crate) inner: &'a mut T,
    pub(crate) font_system: &'a mut FontSystem,
}

impl<'a, T> Deref for BorrowedWithFontSystem<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a, T> DerefMut for BorrowedWithFontSystem<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}
