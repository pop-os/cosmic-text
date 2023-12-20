#[cfg(not(feature = "std"))]
use hashbrown::hash_map::Entry;
#[cfg(feature = "std")]
use std::collections::hash_map::Entry;

use crate::{Font, HashMap};

/// Key for caching shape plans.
#[derive(Debug, Hash, PartialEq, Eq)]
struct ShapePlanKey {
    font_id: fontdb::ID,
    direction: rustybuzz::Direction,
    script: rustybuzz::Script,
    language: Option<rustybuzz::Language>,
}

/// A helper structure for caching rustybuzz shape plans.
#[derive(Default)]
pub struct ShapePlanCache(HashMap<ShapePlanKey, rustybuzz::ShapePlan>);

impl ShapePlanCache {
    pub fn get(&mut self, font: &Font, buffer: &rustybuzz::UnicodeBuffer) -> &rustybuzz::ShapePlan {
        let key = ShapePlanKey {
            font_id: font.id(),
            direction: buffer.direction(),
            script: buffer.script(),
            language: buffer.language(),
        };
        match self.0.entry(key) {
            Entry::Occupied(occ) => occ.into_mut(),
            Entry::Vacant(vac) => {
                let ShapePlanKey {
                    direction,
                    script,
                    language,
                    ..
                } = vac.key();
                let plan = rustybuzz::ShapePlan::new(
                    font.rustybuzz(),
                    *direction,
                    Some(*script),
                    language.as_ref(),
                    &[],
                );
                vac.insert(plan)
            }
        }
    }
}

impl core::fmt::Debug for ShapePlanCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ShapePlanCache").finish()
    }
}
