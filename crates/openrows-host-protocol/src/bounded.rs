// SPDX-License-Identifier: Apache-2.0
//! Decode collections without trusting postcard's external size hints.
use serde::{
    Deserialize, Deserializer,
    de::{Error, SeqAccess, Visitor},
};
use std::{fmt, marker::PhantomData};
struct Bounded<T>(PhantomData<T>);
impl<'de, T: Deserialize<'de>> Visitor<'de> for Bounded<T> {
    type Value = Vec<T>;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("at most 4096 items")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        if sequence.size_hint().is_some_and(|size| size > 4096) {
            return Err(A::Error::custom("collection limit"));
        }
        let mut result = Vec::new();
        while let Some(value) = sequence.next_element()? {
            if result.len() == 4096 {
                return Err(A::Error::custom("collection limit"));
            }
            result.push(value);
        }
        Ok(result)
    }
}
pub(crate) fn list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(Bounded(PhantomData))
}
