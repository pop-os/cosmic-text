#[cfg(not(feature = "std"))]
pub use libm::{roundf, truncf};

#[cfg(feature = "std")]
#[inline]
pub fn roundf(x: f32) -> f32 {
    x.round()
}

#[cfg(feature = "std")]
#[inline]
pub fn truncf(x: f32) -> f32 {
    x.trunc()
}
