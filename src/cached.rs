use core::fmt::Debug;

/// Helper for caching a value when the value is optionally present in the 'unused' state.
#[derive(Clone, Debug)]
pub enum Cached<T: Clone + Debug> {
    Empty,
    Unused(T),
    Used(T),
}

impl<T: Clone + Debug> Cached<T> {
    pub fn get(&self) -> Option<&T> {
        match self {
            Self::Empty | Self::Unused(_) => None,
            Self::Used(t) => Some(t),
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Empty | Self::Unused(_) => None,
            Self::Used(t) => Some(t),
        }
    }

    pub fn is_unused(&self) -> bool {
        match self {
            Self::Empty | Self::Unused(_) => true,
            Self::Used(_) => false,
        }
    }

    pub fn is_used(&self) -> bool {
        match self {
            Self::Empty | Self::Unused(_) => false,
            Self::Used(_) => true,
        }
    }

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

    pub fn set_unused(&mut self) {
        if matches!(*self, Self::Used(_)) {
            *self = Self::Unused(self.take_used().unwrap());
        }
    }

    pub fn set_used(&mut self, val: T) {
        *self = Self::Used(val);
    }
}
