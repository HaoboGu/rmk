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

/// Generate, from a bitfield's bool-bit list, a companion `$flags` struct (the
/// human-readable / TypeScript shape) plus an `is_human_readable`-branching
/// `Serialize`/`Deserialize` for the bitfield: a compact **`u8` on postcard**
/// (byte-identical to the bitfield newtype's derived serde, so the wire is
/// unchanged) and a **struct of named `bool`s on serde_json and
/// serde-wasm-bindgen** (and the matching TS type, under `feature = "wasm"`).
///
/// Drop `Serialize, Deserialize` from the bitfield's own `#[derive(...)]` — this
/// macro provides them. Each `field = setter` entry names the bitfield's
/// generated bool getter and its `with_*` builder; passing both keeps this a
/// plain declarative table (no ident concatenation, no `paste` dependency).
#[macro_export]
macro_rules! flag_bitfield_serde {
    ($bitfield:ident, $flags:ident, { $( $field:ident = $setter:ident ),+ $(,)? }) => {
        #[doc = concat!("Named-boolean view of [`", stringify!($bitfield), "`] — the\n\
            human-readable (JSON / TypeScript) serialization shape.")]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
        #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
        pub struct $flags {
            $(pub $field: bool,)+
        }

        impl serde::Serialize for $bitfield {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                if serializer.is_human_readable() {
                    serde::Serialize::serialize(&$flags { $($field: self.$field(),)+ }, serializer)
                } else {
                    serializer.serialize_u8(self.into_bits())
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $bitfield {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> ::core::result::Result<Self, D::Error> {
                if deserializer.is_human_readable() {
                    let f = <$flags as serde::Deserialize>::deserialize(deserializer)?;
                    ::core::result::Result::Ok(Self::new() $(.$setter(f.$field))+)
                } else {
                    ::core::result::Result::Ok(Self::from_bits(<u8 as serde::Deserialize>::deserialize(deserializer)?))
                }
            }
        }
    };
}
