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

/// Bridges Rust `#[bitfield(u8)]` types with non-bitfield TypeScript objects.
/// With `feature = "wasm"`, emits the TypeScript object type.
///
/// Also implements Serialize/Deserialize so Rust values can be converted to
/// TypeScript objects seamlessly.
#[macro_export]
macro_rules! bitfield_named_serde {
    ($bitfield:ident, { $( $field:ident = $setter:ident ),+ $(,)? }) => {
        impl serde::Serialize for $bitfield {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                if serializer.is_human_readable() {
                    #[derive(serde::Serialize)]
                    struct Repr { $($field: bool,)+ }
                    serde::Serialize::serialize(&Repr { $($field: self.$field(),)+ }, serializer)
                } else {
                    serializer.serialize_u8(self.into_bits())
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $bitfield {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> ::core::result::Result<Self, D::Error> {
                if deserializer.is_human_readable() {
                    #[derive(serde::Deserialize)]
                    struct Repr { $($field: bool,)+ }
                    let r = <Repr as serde::Deserialize>::deserialize(deserializer)?;
                    ::core::result::Result::Ok(Self::new() $(.$setter(r.$field))+)
                } else {
                    ::core::result::Result::Ok(Self::from_bits(<u8 as serde::Deserialize>::deserialize(deserializer)?))
                }
            }
        }

        // Static `.d.ts` shape, built from the same field table.
        #[cfg(feature = "wasm")]
        const _: () = {
            #[::wasm_bindgen::prelude::wasm_bindgen(typescript_custom_section)]
            const TS_APPEND_CONTENT: &'static str = concat!(
                "export type ", stringify!($bitfield), " = {",
                $( " ", stringify!($field), ": boolean;", )+
                " };"
            );
        };
    };
}
