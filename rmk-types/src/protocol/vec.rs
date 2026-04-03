//! A Vec type that adapts to the compilation target:
//! - **Firmware** (`#[cfg(not(feature = "host"))]`): type alias for `heapless::Vec<T, N>` —
//!   stack-allocated, capacity bounded by `N`.
//! - **Host** (`#[cfg(feature = "host")]`): newtype around `alloc::Vec<T>` —
//!   heap-allocated, `N` is used only for `MaxSize` computation and `Schema` compatibility.
//!
//! Both produce identical postcard wire format (`varint(len) + elements`).

// ---------------------------------------------------------------------------
// Firmware: zero-cost alias for heapless::Vec
// ---------------------------------------------------------------------------
#[cfg(not(feature = "host"))]
pub type Vec<T, const N: usize> = heapless::Vec<T, N>;

// ---------------------------------------------------------------------------
// Host: newtype around alloc::Vec that ignores N at runtime
// ---------------------------------------------------------------------------
#[cfg(feature = "host")]
mod host_vec {
    extern crate alloc;

    use alloc::vec::Vec;
    use core::fmt;
    use core::ops::{Deref, DerefMut};

    use postcard::experimental::max_size::MaxSize;
    use postcard_schema::Schema;
    use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
    use serde::ser::{Serialize, SerializeSeq, Serializer};

    /// A heap-allocated protocol Vec. `N` is retained as a type parameter for
    /// `MaxSize` / `Schema` compatibility with the firmware's `heapless::Vec<T, N>`,
    /// but does **not** limit the runtime capacity.
    pub struct Vec<T, const N: usize>(alloc::vec::Vec<T>);

    // -- Construction & mutation --

    impl<T, const N: usize> self::Vec<T, N> {
        pub fn new() -> Self {
            Self(alloc::vec::Vec::new())
        }

        /// Push an element. Always succeeds on the host (unbounded capacity).
        /// Signature matches `heapless::Vec::push` for source compatibility.
        pub fn push(&mut self, item: T) -> Result<(), T> {
            self.0.push(item);
            Ok(())
        }

        /// Extend from a slice. Always succeeds on the host.
        pub fn extend_from_slice(&mut self, other: &[T]) -> Result<(), ()>
        where
            T: Clone,
        {
            self.0.extend_from_slice(other);
            Ok(())
        }

        pub fn truncate(&mut self, len: usize) {
            self.0.truncate(len);
        }

        pub fn clear(&mut self) {
            self.0.clear();
        }

        pub fn capacity(&self) -> usize {
            self.0.capacity()
        }
    }

    // -- Deref / DerefMut → [T] --

    impl<T, const N: usize> Deref for self::Vec<T, N> {
        type Target = [T];
        fn deref(&self) -> &[T] {
            &self.0
        }
    }

    impl<T, const N: usize> DerefMut for self::Vec<T, N> {
        fn deref_mut(&mut self) -> &mut [T] {
            &mut self.0
        }
    }

    // -- Standard traits --

    impl<T: Clone, const N: usize> Clone for self::Vec<T, N> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<T: fmt::Debug, const N: usize> fmt::Debug for self::Vec<T, N> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }

    impl<T: PartialEq, const N: usize> PartialEq for self::Vec<T, N> {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl<T: Eq, const N: usize> Eq for self::Vec<T, N> {}

    impl<T, const N: usize> Default for self::Vec<T, N> {
        fn default() -> Self {
            Self::new()
        }
    }

    // -- Iterator support --

    impl<T, const N: usize> FromIterator<T> for self::Vec<T, N> {
        fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
            Self(iter.into_iter().collect())
        }
    }

    // -- Serde: identical wire format to heapless::Vec --

    impl<T: Serialize, const N: usize> Serialize for self::Vec<T, N> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
            for item in &self.0 {
                seq.serialize_element(item)?;
            }
            seq.end()
        }
    }

    impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for self::Vec<T, N> {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct VecVisitor<T, const N: usize>(core::marker::PhantomData<T>);

            impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for VecVisitor<T, N> {
                type Value = self::super::host_vec::Vec<T, N>;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "a sequence")
                }

                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                    let mut vec = alloc::vec::Vec::with_capacity(seq.size_hint().unwrap_or(0));
                    while let Some(elem) = seq.next_element()? {
                        vec.push(elem);
                    }
                    Ok(super::host_vec::Vec(vec))
                }
            }

            deserializer.deserialize_seq(VecVisitor::<T, N>(core::marker::PhantomData))
        }
    }

    // -- MaxSize: uses N as the upper bound (same formula as heapless::Vec) --

    impl<T: MaxSize, const N: usize> MaxSize for self::Vec<T, N> {
        const POSTCARD_MAX_SIZE: usize =
            T::POSTCARD_MAX_SIZE * N + crate::varint_max_size(N);
    }

    // -- Schema: delegate to heapless::Vec so endpoint keys match firmware --

    impl<T: Schema, const N: usize> Schema for self::Vec<T, N> {
        const SCHEMA: &'static postcard_schema::schema::NamedType =
            <heapless::Vec<T, N> as Schema>::SCHEMA;
    }

    // -- defmt (optional) --

    #[cfg(feature = "defmt")]
    impl<T: defmt::Format, const N: usize> defmt::Format for self::Vec<T, N> {
        fn format(&self, f: defmt::Formatter<'_>) {
            defmt::write!(f, "[");
            for (i, item) in self.0.iter().enumerate() {
                if i > 0 {
                    defmt::write!(f, ", ");
                }
                defmt::write!(f, "{:?}", item);
            }
            defmt::write!(f, "]");
        }
    }
}

#[cfg(feature = "host")]
pub use host_vec::Vec;
