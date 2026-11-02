// SPDX-License-Identifier: Apache-2.0
//! JSON5 values must not silently turn NaN/Infinity into null or round integers.
use super::ParseError;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use std::fmt;

struct Checked(Value);
impl<'de> Deserialize<'de> for Checked {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(CheckedVisitor)
    }
}
struct CheckedVisitor;
impl<'de> Visitor<'de> for CheckedVisitor {
    type Value = Checked;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON/EJSON with exact BSON-compatible numbers")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Checked, E> {
        Ok(Checked(Value::Null))
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Checked, E> {
        Ok(Checked(Value::Bool(value)))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Checked, E> {
        Ok(Checked(Value::Number(value.into())))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Checked, E> {
        let value = i64::try_from(value)
            .map_err(|_| E::custom("use EJSON for integers outside signed 64-bit range"))?;
        self.visit_i64(value)
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Checked, E> {
        Number::from_f64(value)
            .map(|n| Checked(Value::Number(n)))
            .ok_or_else(|| E::custom("use EJSON for non-finite numbers"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Checked, E> {
        Ok(Checked(Value::String(value.into())))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Checked, E> {
        Ok(Checked(Value::String(value)))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Checked, A::Error> {
        let mut values = Vec::new();
        while let Some(Checked(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(Checked(Value::Array(values)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Checked, A::Error> {
        let mut values = Map::new();
        while let Some((key, Checked(value))) = map.next_entry::<String, Checked>()? {
            if values.insert(key, value).is_some() {
                return Err(de::Error::custom("duplicate document field"));
            }
        }
        Ok(Checked(Value::Object(values)))
    }
}
/// Decode literal arguments with strict numeric and duplicate-field validation.
pub(super) fn arguments(text: &str) -> Result<Vec<Value>, ParseError> {
    json5::from_str::<Vec<Checked>>(text)
        .map(|values| values.into_iter().map(|value| value.0).collect())
        .map_err(|_| ParseError("invalid JSON/EJSON, duplicate fields or non-BSON numbers"))
}
pub(super) fn literal(text: &str) -> Result<Value, ParseError> {
    json5::from_str::<Checked>(text)
        .map(|value| value.0)
        .map_err(|_| ParseError("invalid BSON literal"))
}
pub(crate) fn document(text: &str) -> Result<Value, ParseError> {
    super::scan::validate_size(text)?;
    serde_json::from_str::<de::IgnoredAny>(text)
        .map_err(|_| ParseError("expected valid JSON/EJSON"))?;
    literal(text)
}
