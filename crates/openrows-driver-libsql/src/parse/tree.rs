// SPDX-License-Identifier: Apache-2.0
//! Recursive envelope visitor; only row arrays take the streaming path.
use super::{
    Event,
    rows::{Deferred, Publisher, Rows},
    send,
};
use crate::ResultLimits;
use openrows_driver_sqlite::decode::ColumnSpec;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
pub(super) struct Tree {
    pub(super) events: mpsc::Sender<Event>,
    pub(super) limits: ResultLimits,
    pub(super) scope: Scope,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Scope {
    Pipeline,
    Results,
    Result,
    Response,
    Statement,
    Statements,
    Other,
}

impl Scope {
    fn field(self, key: &str) -> Self {
        match (self, key) {
            (Self::Pipeline, "results") => Self::Results,
            (Self::Result, "response") => Self::Response,
            (Self::Response, "result") => Self::Statement,
            (Self::Statement, "step_results") => Self::Statements,
            _ => Self::Other,
        }
    }

    fn element(self) -> Self {
        match self {
            Self::Results => Self::Result,
            Self::Statements => Self::Statement,
            _ => Self::Other,
        }
    }
}

impl<'de> DeserializeSeed<'de> for Tree {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Tree {
    type Value = Value;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Hrana envelope")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.into()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        let element = Self {
            scope: self.scope.element(),
            ..self
        };
        while let Some(value) = sequence.next_element_seed(element.clone())? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut fields = Map::new();
        let mut deferred = None;
        while let Some(key) = map.next_key::<String>()? {
            if fields.contains_key(&key) {
                return Err(de::Error::custom("duplicate Hrana field"));
            }
            let value = if key == "rows" && self.scope == Scope::Statement {
                let columns = columns(&fields).map_err(de::Error::custom)?;
                deferred = Some(map.next_value_seed(Rows {
                    columns,
                    events: &self.events,
                    cap: self.limits.rows,
                })?);
                Value::Array(Vec::new())
            } else {
                map.next_value_seed(Self {
                    scope: self.scope.field(&key),
                    ..self.clone()
                })?
            };
            fields.insert(key, value);
        }
        if let Some(deferred) = deferred {
            self.finish(&fields, deferred).map_err(de::Error::custom)?;
        }
        Ok(Value::Object(fields))
    }
}

impl Tree {
    fn finish(
        &self,
        fields: &Map<String, Value>,
        deferred: Deferred,
    ) -> Result<(), openrows_driver_api::DriverError> {
        if !deferred.streamed {
            let columns = columns(fields)?.ok_or_else(crate::error::protocol)?;
            let mut publisher = Publisher::new(columns, &self.events);
            for row in deferred.rows {
                publisher.push(row)?;
            }
            publisher.finish()?;
        }
        if deferred.truncated {
            send(&self.events, Event::Truncated(self.limits.rows))?;
        }
        Ok(())
    }
}

fn columns(
    fields: &Map<String, Value>,
) -> Result<Option<Arc<[ColumnSpec]>>, openrows_driver_api::DriverError> {
    let Some(cols) = fields.get("cols") else {
        return Ok(None);
    };
    let cols = cols.as_array().ok_or_else(crate::error::protocol)?;
    let columns = cols
        .iter()
        .enumerate()
        .map(|(index, col)| ColumnSpec {
            name: col["name"]
                .as_str()
                .map_or_else(|| format!("column{}", index + 1), str::to_owned),
            declared: col["decltype"].as_str().map(str::to_owned),
        })
        .collect();
    Ok(Some(columns))
}
