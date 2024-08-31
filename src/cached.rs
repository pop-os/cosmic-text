use core::fmt::Debug;

/// Helper for caching a value when the value is optionally present in the 'unused' state.
#[derive(Clone, Debug)]
pub enum Cached<T: Clone + Debug> {
    Empty,
    Unused(T),
    Used(T),
}

impl<T: Clone + Debug> Cached<T> {
    /// Gets the value if in state `Self::Used`.
    pub fn get(&self) -> Option<&T> {
        match self {
            Self::Empty | Self::Unused(_) => None,
            Self::Used(t) => Some(t),
        }
    }

    /// Gets the value mutably if in state `Self::Used`.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Empty | Self::Unused(_) => None,
            Self::Used(t) => Some(t),
        }
    }

    /// Checks if the value is empty or unused.
    pub fn is_unused(&self) -> bool {
        match self {
            Self::Empty | Self::Unused(_) => true,
            Self::Used(_) => false,
        }
    }

    /// Checks if the value is used (i.e. cached for access).
    pub fn is_used(&self) -> bool {
        match self {
            Self::Empty | Self::Unused(_) => false,
            Self::Used(_) => true,
        }
    }

    /// Takes the buffered value if in state `Self::Unused`.
    pub fn take_unused(&mut self) -> Option<T> {
        if matches!(*self, Self::Unused(_)) {
            let Self::Unused(val) = std::mem::replace(self, Self::Empty) else {
                unreachable!()
            };
            Some(val)
        } else {
            None
        }
    }

    /// Takes the cached value if in state `Self::Used`.
    pub fn take_used(&mut self) -> Option<T> {
        if matches!(*self, Self::Used(_)) {
            let Self::Used(val) = std::mem::replace(self, Self::Empty) else {
                unreachable!()
            };
            Some(val)
        } else {
            None
        }
    }

    /// Moves the value from `Self::Used` to `Self::Unused`.
    pub fn set_unused(&mut self) {
        if matches!(*self, Self::Used(_)) {
            *self = Self::Unused(self.take_used().unwrap());
        }
    }

    /// Sets the value to `Self::Used`.
    pub fn set_used(&mut self, val: T) {
        *self = Self::Used(val);
    }
}
