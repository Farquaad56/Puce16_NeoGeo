//! Sérialisation serde des tableaux de taille quelconque (serde s'arrête à 32 éléments).

use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
    a: &[T; N], // Box<[T; N]> par déréférencement
    s: S,
) -> Result<S::Ok, S::Error> {
    a.as_slice().serialize(s)
}

pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
    d: D,
) -> Result<Box<[T; N]>, D::Error> {
    Vec::<T>::deserialize(d)?
        .into_boxed_slice()
        .try_into()
        .map_err(|_| D::Error::custom("taille de tableau"))
}
