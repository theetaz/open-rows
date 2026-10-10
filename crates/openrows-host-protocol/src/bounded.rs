// SPDX-License-Identifier: Apache-2.0
//! Decode collections without trusting postcard's external size hints.
use serde::{
    Deserialize, Deserializer,
    de::{Error, SeqAccess, Visitor},
};
use std::{fmt, marker::PhantomData};
struct Bounded<T, const LIMIT: usize>(PhantomData<T>);
impl<'de, T: Deserialize<'de>, const LIMIT: usize> Visitor<'de> for Bounded<T, LIMIT> {
    type Value = Vec<T>;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "at most {LIMIT} items")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        if sequence.size_hint().is_some_and(|size| size > LIMIT) {
            return Err(A::Error::custom("collection limit"));
        }
        let hint = sequence.size_hint();
        if let Some(items) = hint {
            crate::allocation::charge::<T>(items)
                .map_err(|()| A::Error::custom("aggregate collection limit"))?;
        }
        let mut result = Vec::new();
        loop {
            if hint.is_none() {
                crate::allocation::charge::<T>(1)
                    .map_err(|()| A::Error::custom("aggregate collection limit"))?;
            }
            let Some(value) = sequence.next_element()? else {
                break;
            };
            if result.len() == LIMIT {
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
    deserializer.deserialize_seq(Bounded::<T, 4096>(PhantomData))
}

#[derive(serde::Deserialize)]
struct Row(#[serde(deserialize_with = "list")] Vec<Option<String>>);
pub(crate) fn rows<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Vec<Option<String>>>, D::Error> {
    let rows: Vec<Row> = list(deserializer)?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

pub(crate) fn catalog<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(Bounded::<T, 65536>(PhantomData))
}
pub(crate) fn chunk<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    deserializer.deserialize_seq(Bounded::<u8, 65536>(PhantomData))
}
