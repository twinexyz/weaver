use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::serde_utils::{hex_fixed_vec, hex_var_list};
use ssz_types::{FixedVector, VariableList};
use tree_hash_derive::TreeHash;

macro_rules! define_byte_container {
    ($name:ident, $inner_type:ident, $serde_hex_module:ident) => {
        #[derive(Debug, Clone, Default, Encode, Decode, TreeHash)]
        #[ssz(struct_behaviour = "transparent")]
        pub struct $name<N: typenum::Unsigned> {
            pub data: $inner_type<u8, N>,
        }

        impl<N: typenum::Unsigned> $name<N> {
            pub fn from_slice(data: &[u8]) -> Result<Self, ssz_types::Error> {
                let inner = $inner_type::<u8, N>::try_from(data.to_vec()).unwrap();
                Ok(Self { data: inner })
            }
        }

        impl<'de, N: typenum::Unsigned> Deserialize<'de> for $name<N> {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>, {
                $serde_hex_module::deserialize(deserializer).map(|data| Self { data })
            }
        }

        impl<N: typenum::Unsigned> Serialize for $name<N> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer, {
                $serde_hex_module::serialize(&self.data, serializer)
            }
        }
    };
}

define_byte_container!(ByteVector, FixedVector, hex_fixed_vec);
define_byte_container!(ByteList, VariableList, hex_var_list);
