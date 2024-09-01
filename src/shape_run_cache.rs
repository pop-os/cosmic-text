#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::ops::Range;

use crate::{AttrsOwned, HashMap, HashSet, ShapeGlyph};

/// Key for caching shape runs.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ShapeRunKey {
    pub text: String,
    pub default_attrs: AttrsOwned,
    pub attrs_spans: Vec<(Range<usize>, AttrsOwned)>,
}

/// A helper structure for caching shape runs.
#[derive(Clone)]
pub struct ShapeRunCache {
    age: u64,
    cache: HashMap<ShapeRunKey, (u64, Vec<ShapeGlyph>)>,
    age_registries: Vec<HashSet<ShapeRunKey>>,
}

impl Default for ShapeRunCache {
    fn default() -> Self {
        Self {
            age: 0,
            cache: Default::default(),
            age_registries: vec![HashSet::default()],
        }
    }
}

impl ShapeRunCache {
    /// Get cache item, updating age if found
    pub fn get(&mut self, key: &ShapeRunKey) -> Option<&Vec<ShapeGlyph>> {
        self.cache.get_mut(key).map(|(age, glyphs)| {
            if *age != self.age {
                // remove the key from the old age registry
                let index = (self.age - *age) as usize;
                self.age_registries[index].remove(key);

                // update age
                *age = self.age;
                // register the key to the new age registry
                if let Some(keys) = self.age_registries.first_mut() {
                    keys.insert(key.clone());
                }
            }
            &*glyphs
        })
    }

    /// Insert cache item with current age
    pub fn insert(&mut self, key: ShapeRunKey, glyphs: Vec<ShapeGlyph>) {
        if let Some(keys) = self.age_registries.first_mut() {
            // register the key to the current age
            keys.insert(key.clone());
        }
        self.cache.insert(key, (self.age, glyphs));
    }

    /// Remove anything in the cache with an age older than keep_ages
    pub fn trim(&mut self, keep_ages: u64) {
        // remove the age registries that's greater than kept ages
        // and remove the keys from cache saved in the registries
        while self.age_registries.len() as u64 > keep_ages {
            if let Some(keys) = self.age_registries.pop() {
                for key in keys {
                    self.cache.remove(&key);
                }
            }
        }
        // Increase age
        self.age += 1;
        // insert a new registry to the front of the Vec
        // to keep keys for the current age
        self.age_registries.insert(0, HashSet::default());
    }
}

impl core::fmt::Debug for ShapeRunCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ShapeRunCache").finish()
    }
}

#[cfg(test)]
mod test {
    use crate::{Attrs, AttrsOwned, ShapeRunCache, ShapeRunKey};

    #[test]
    fn test_trim() {
        let key1 = ShapeRunKey {
            text: "1".to_string(),
            default_attrs: AttrsOwned::new(Attrs::new()),
            attrs_spans: Vec::new(),
        };

        let key2 = ShapeRunKey {
            text: "2".to_string(),
            default_attrs: AttrsOwned::new(Attrs::new()),
            attrs_spans: Vec::new(),
        };

        let key3 = ShapeRunKey {
            text: "3".to_string(),
            default_attrs: AttrsOwned::new(Attrs::new()),
            attrs_spans: Vec::new(),
        };

        let mut cache = ShapeRunCache::default();

        cache.insert(key1.clone(), Vec::new());
        cache.insert(key2.clone(), Vec::new());
        cache.insert(key3.clone(), Vec::new());
        // this will trim everything
        cache.trim(0);
        assert!(cache.cache.is_empty());

        cache.insert(key1.clone(), Vec::new());
        cache.insert(key2.clone(), Vec::new());
        cache.insert(key3.clone(), Vec::new());
        // keep 1 age
        cache.trim(1);
        // all was just inserted so all kept
        assert_eq!(cache.cache.len(), 3);
        assert_eq!(cache.age_registries.len(), 2);

        cache.get(&key1);
        cache.get(&key2);
        cache.trim(1);
        // only key1 and key2 was refreshed, so key3 was trimed
        assert_eq!(cache.cache.len(), 2);
        assert_eq!(cache.age_registries.len(), 2);

        cache.get(&key1);
        cache.trim(1);
        // only key1 was refreshed, so key2 was trimed
        assert_eq!(cache.cache.len(), 1);
        assert_eq!(cache.age_registries.len(), 2);

        cache.trim(2);
        // keep 2 ages, so even key1 wasn't refreshed,
        // it was still kept
        assert_eq!(cache.cache.len(), 1);
        assert_eq!(cache.age_registries.len(), 3);

        cache.trim(2);
        // key1 is now too old for 2 ages, so it was trimed
        assert_eq!(cache.cache.len(), 0);
        assert_eq!(cache.age_registries.len(), 3);
    }
}
