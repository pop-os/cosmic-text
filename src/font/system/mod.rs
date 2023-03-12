use core::ops::{Deref, DerefMut};

#[cfg(not(feature = "std"))]
pub use self::no_std::*;
#[cfg(not(feature = "std"))]
mod no_std;

#[cfg(feature = "std")]
pub use self::std::*;
#[cfg(feature = "std")]
mod std;

// re-export fontdb
pub use fontdb;

pub struct BorrowedWithFontSystem<'a, T> {
    pub(crate) inner: &'a mut T,
    pub(crate) font_system: &'a FontSystem,
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
