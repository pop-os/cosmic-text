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
