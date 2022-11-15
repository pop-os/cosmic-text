#[cfg(not(feature = "std"))]
pub use self::no_std::*;
#[cfg(not(feature = "std"))]
mod no_std;

//TODO: use std implementation on Redox
#[cfg(all(feature = "std", target_os = "redox"))]
pub use self::redox::*;
#[cfg(all(feature = "std", target_os = "redox"))]
mod redox;

#[cfg(all(feature = "std", not(target_os = "redox")))]
pub use self::std::*;
#[cfg(all(feature = "std", not(target_os = "redox")))]
mod std;
