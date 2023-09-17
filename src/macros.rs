/// Used for creating container type that self-reference owned data,
/// see also <https://morestina.net/blog/1868/self-referential-types-for-fun-and-profit>
///
/// # Example
///
/// ```ignore
/// struct Data();
/// struct SomeRef<'a>(&'a Data);
///
/// mod inner {
///     impl_self_ref!(Container, SomeRef<'static>, SomeRef<'this>);
/// }
/// use inner::Container;
///
/// let container = Container::new(Data(), |data| SomeRef(data));
/// let some_ref = container.as_ref();
/// ```
macro_rules! impl_self_ref {
    ($SelfRef:ident, $RefStatic:ty, $RefThis:ty) => {
        /// # Safety invariant
        ///
        /// `data` must never have a mutable reference taken, nor be modified during the lifetime of
        /// this struct
        pub struct $SelfRef<T> {
            /// `data_ref` could self-referencing `data`
            data_ref: $RefStatic,
            /// `data` field must be after `data_ref` to ensure that it is dropped later
            data: ::aliasable::boxed::AliasableBox<T>,
        }
        impl<T> ::core::fmt::Debug for $SelfRef<T> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(stringify!($SelfRef)).finish_non_exhaustive()
            }
        }

        impl<'this, T> AsRef<$RefThis> for $SelfRef<T>
        where
            Self: 'this,
        {
            fn as_ref(&self) -> &$RefThis {
                &self.data_ref
            }
        }

        impl<T> $SelfRef<T> {
            pub fn new<F>(data: T, builder: F) -> Option<Self>
            where
                T: 'static,
                for<'this> F: FnOnce(&'this T) -> Option<$RefThis>,
            {
                unsafe fn change_lifetime<'old, 'new: 'old, T: 'new>(data: &'old T) -> &'new T {
                    &*(data as *const _)
                }
                let data =
                    ::aliasable::boxed::AliasableBox::from_unique(::alloc::boxed::Box::new(data));
                let data_ref = builder(unsafe { change_lifetime(&data) })?;
                Some(Self { data_ref, data })
            }

            /// # Safety
            ///
            /// Allows immutable access to data only
            #[allow(dead_code)]
            pub fn as_backing_data(&self) -> &T {
                &self.data
            }
        }
    };
}
