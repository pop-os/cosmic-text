use crate::{Attrs, Font, FontMatchAttrs, HashMap, ShapePlanCache};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::ops::{Deref, DerefMut};

// re-export fontdb and rustybuzz
pub use fontdb;
pub use rustybuzz;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontMatchKey {
    pub(crate) font_weight_diff: u16,
    pub(crate) font_weight: u16,
    pub(crate) id: fontdb::ID,
}

struct FontCachedCodepointSupportInfo {
    supported: Vec<u32>,
    not_supported: Vec<u32>,
}

impl FontCachedCodepointSupportInfo {
    const SUPPORTED_MAX_SZ: usize = 512;
    const NOT_SUPPORTED_MAX_SZ: usize = 1024;

    fn new() -> Self {
        Self {
            supported: Vec::with_capacity(Self::SUPPORTED_MAX_SZ),
            not_supported: Vec::with_capacity(Self::NOT_SUPPORTED_MAX_SZ),
        }
    }

    #[inline(always)]
    fn unknown_has_codepoint(
        &mut self,
        font_codepoints: &[u32],
        codepoint: u32,
        supported_insert_pos: usize,
        not_supported_insert_pos: usize,
    ) -> bool {
        let ret = font_codepoints.contains(&codepoint);
        if ret {
            // don't bother inserting if we are going to truncate the entry away
            if supported_insert_pos != Self::SUPPORTED_MAX_SZ {
                self.supported.insert(supported_insert_pos, codepoint);
                self.supported.truncate(Self::SUPPORTED_MAX_SZ);
            }
        } else {
            // don't bother inserting if we are going to truncate the entry away
            if not_supported_insert_pos != Self::NOT_SUPPORTED_MAX_SZ {
                self.not_supported
                    .insert(not_supported_insert_pos, codepoint);
                self.not_supported.truncate(Self::NOT_SUPPORTED_MAX_SZ);
            }
        }
        ret
    }

    #[inline(always)]
    fn has_codepoint(&mut self, font_codepoints: &[u32], codepoint: u32) -> bool {
        match self.supported.binary_search(&codepoint) {
            Ok(_) => true,
            Err(supported_insert_pos) => match self.not_supported.binary_search(&codepoint) {
                Ok(_) => false,
                Err(not_supported_insert_pos) => self.unknown_has_codepoint(
                    font_codepoints,
                    codepoint,
                    supported_insert_pos,
                    not_supported_insert_pos,
                ),
            },
        }
    }
}

/// Access to the system fonts.
pub struct FontSystem {
    /// The locale of the system.
    locale: String,

    /// The underlying font database.
    db: fontdb::Database,

    /// Cache for loaded fonts from the database.
    font_cache: HashMap<fontdb::ID, Option<Arc<Font>>>,

    /// Sorted unique ID's of all Monospace fonts in DB
    monospace_font_ids: Vec<fontdb::ID>,

    /// Sorted unique ID's of all Monospace fonts in DB per script.
    /// A font may support multiple scripts of course, so the same ID
    /// may appear in multiple map value vecs.
    per_script_monospace_font_ids: HashMap<[u8; 4], Vec<fontdb::ID>>,

    /// Cache for font codepoint support info
    font_codepoint_support_info_cache: HashMap<fontdb::ID, FontCachedCodepointSupportInfo>,

    /// Cache for font matches.
    font_matches_cache: HashMap<FontMatchAttrs, Arc<Vec<FontMatchKey>>>,

    /// Cache for rustybuzz shape plans.
    shape_plan_cache: ShapePlanCache,

    /// Cache for shaped runs
    #[cfg(feature = "shape-run-cache")]
    pub shape_run_cache: crate::ShapeRunCache,
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
    const FONT_MATCHES_CACHE_SIZE_LIMIT: usize = 256;
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
        let mut monospace_font_ids = db
            .faces()
            .filter(|face_info| {
                face_info.monospaced && !face_info.post_script_name.contains("Emoji")
            })
            .map(|face_info| face_info.id)
            .collect::<Vec<_>>();
        monospace_font_ids.sort();

        let cloned_monospace_font_ids = monospace_font_ids.clone();

        let mut ret = Self {
            locale,
            db,
            monospace_font_ids,
            per_script_monospace_font_ids: Default::default(),
            font_cache: Default::default(),
            font_matches_cache: Default::default(),
            font_codepoint_support_info_cache: Default::default(),
            shape_plan_cache: ShapePlanCache::default(),
            #[cfg(feature = "shape-run-cache")]
            shape_run_cache: crate::ShapeRunCache::default(),
        };
        ret.cache_fonts(cloned_monospace_font_ids.clone());
        cloned_monospace_font_ids.into_iter().for_each(|id| {
            if let Some(font) = ret.get_font(id) {
                font.scripts().iter().copied().for_each(|script| {
                    ret.per_script_monospace_font_ids
                        .entry(script)
                        .or_default()
                        .push(font.id);
                });
            }
        });
        ret
    }

    /// Get the locale.
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Get the database.
    pub fn db(&self) -> &fontdb::Database {
        &self.db
    }

    /// Get the shape plan cache.
    pub(crate) fn shape_plan_cache(&mut self) -> &mut ShapePlanCache {
        &mut self.shape_plan_cache
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

    /// Concurrently cache fonts by id list
    pub fn cache_fonts(&mut self, mut ids: Vec<fontdb::ID>) {
        #[cfg(feature = "std")]
        use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
        #[cfg(feature = "std")]
        {
            ids = ids
                .into_iter()
                .filter(|id| {
                    let contains = self.font_cache.contains_key(id);
                    if !contains {
                        unsafe {
                            self.db.make_shared_face_data(*id);
                        }
                    }
                    !contains
                })
                .collect::<_>();
        }

        #[cfg(feature = "std")]
        let fonts = ids.par_iter();
        #[cfg(not(feature = "std"))]
        let fonts = ids.iter();

        fonts
            .map(|id| match Font::new(&self.db, *id) {
                Some(font) => Some(Arc::new(font)),
                None => {
                    log::warn!(
                        "failed to load font '{}'",
                        self.db.face(*id)?.post_script_name
                    );
                    None
                }
            })
            .collect::<Vec<Option<Arc<Font>>>>()
            .into_iter()
            .flatten()
            .for_each(|font| {
                self.font_cache.insert(font.id, Some(font));
            });
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
                match Font::new(&self.db, id) {
                    Some(font) => Some(Arc::new(font)),
                    None => {
                        log::warn!(
                            "failed to load font '{}'",
                            self.db.face(id)?.post_script_name
                        );
                        None
                    }
                }
            })
            .clone()
    }

    pub fn is_monospace(&self, id: fontdb::ID) -> bool {
        self.monospace_font_ids.binary_search(&id).is_ok()
    }

    pub fn get_monospace_ids_for_scripts(
        &self,
        scripts: impl Iterator<Item = [u8; 4]>,
    ) -> Vec<fontdb::ID> {
        let mut ret = scripts
            .filter_map(|script| self.per_script_monospace_font_ids.get(&script))
            .flat_map(|ids| ids.iter().copied())
            .collect::<Vec<_>>();
        ret.sort();
        ret.dedup();
        ret
    }

    #[inline(always)]
    pub fn get_font_supported_codepoints_in_word(
        &mut self,
        id: fontdb::ID,
        word: &str,
    ) -> Option<usize> {
        self.get_font(id).map(|font| {
            let code_points = font.unicode_codepoints();
            let cache = self
                .font_codepoint_support_info_cache
                .entry(id)
                .or_insert_with(FontCachedCodepointSupportInfo::new);
            word.chars()
                .filter(|ch| cache.has_codepoint(code_points, u32::from(*ch)))
                .count()
        })
    }

    pub fn get_font_matches(&mut self, attrs: Attrs<'_>) -> Arc<Vec<FontMatchKey>> {
        // Clear the cache first if it reached the size limit
        if self.font_matches_cache.len() >= Self::FONT_MATCHES_CACHE_SIZE_LIMIT {
            log::trace!("clear font mache cache");
            self.font_matches_cache.clear();
        }

        self.font_matches_cache
            //TODO: do not create AttrsOwned unless entry does not already exist
            .entry(attrs.into())
            .or_insert_with(|| {
                #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
                let now = std::time::Instant::now();

                let mut font_match_keys = self
                    .db
                    .faces()
                    .filter(|face| attrs.matches(face))
                    .map(|face| FontMatchKey {
                        font_weight_diff: attrs.weight.0.abs_diff(face.weight.0),
                        font_weight: face.weight.0,
                        id: face.id,
                    })
                    .collect::<Vec<_>>();

                // Sort so we get the keys with weight_offset=0 first
                font_match_keys.sort();

                #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
                {
                    let elapsed = now.elapsed();
                    log::debug!("font matches for {:?} in {:?}", attrs, elapsed);
                }

                Arc::new(font_match_keys)
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

        db.load_system_fonts();

        for source in fonts {
            db.load_font_source(source);
        }

        #[cfg(not(target_arch = "wasm32"))]
        log::debug!(
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
