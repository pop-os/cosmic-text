#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{Feature, HashMap};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ShapePlanKey {
    pub font: fontdb::ID,
    pub direction: harfrust::Direction,
    pub script: harfrust::Script,
    pub language: Option<harfrust::Language>,
    pub features: Vec<Feature>,
}

/// A helper structure for caching shape plans.
#[derive(Default)]
pub struct ShapePlanCache {
    age: u64,
    cache: HashMap<ShapePlanKey, (u64, harfrust::ShapePlan)>,
}

impl ShapePlanCache {
    /// Get or insert cache item, updating age if found
    pub(crate) fn get_or_insert_with(
        &mut self,
        key: ShapePlanKey,
        default: impl FnOnce() -> harfrust::ShapePlan,
    ) -> &harfrust::ShapePlan {
        &self
            .cache
            .entry(key)
            .and_modify(|(age, _)| {
                *age = self.age;
            })
            .or_insert_with(|| (self.age, default()))
            .1
    }

    /// Remove anything in the cache with an age older than `keep_ages`
    pub fn trim(&mut self, keep_ages: u64) {
        self.cache
            .retain(|_key, (age, _shape_plan)| *age + keep_ages >= self.age);
        // Increase age
        self.age += 1;
    }
}

impl core::fmt::Debug for ShapePlanCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ShapePlanCache").finish()
    }
}
