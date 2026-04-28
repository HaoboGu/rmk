//! Formatting helpers shared across the RMK crates.

/// Implements `core::fmt::Debug` and (under `feature = "defmt"`) `defmt::Format`
/// for `$ty`, rendering as a list whose entries come from iterating `$iter`.
///
/// `$iter` is evaluated in a scope where `$self` is bound to `&self`. Items
/// must implement `Debug` and, when `defmt` is enabled, `defmt::Format`.
///
/// One invocation keeps the log and defmt renderings from drifting apart.
#[macro_export]
macro_rules! impl_debug_list {
    ($ty:ty, |$self:ident| $iter:expr $(,)?) => {
        impl ::core::fmt::Debug for $ty {
            fn fmt(&$self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_list().entries($iter).finish()
            }
        }
        #[cfg(feature = "defmt")]
        impl ::defmt::Format for $ty {
            fn format(&$self, f: ::defmt::Formatter) {
                ::defmt::write!(f, "[");
                let mut first = true;
                for v in $iter {
                    if first {
                        first = false;
                    } else {
                        ::defmt::write!(f, ", ");
                    }
                    ::defmt::write!(f, "{}", v);
                }
                ::defmt::write!(f, "]");
            }
        }
    };
}
