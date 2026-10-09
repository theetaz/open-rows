// SPDX-License-Identifier: Apache-2.0
//! Snapshot payloads: versioned JSON of one listing. A payload this build
//! cannot read (older format, corrupt row) decodes to `None`, which only
//! means the next read goes to the server.

use openrows_driver_api::catalog::{DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary};
use serde::{Deserialize, Serialize};

use crate::index::NameIndex;
use crate::listing::Listing;

/// Bumped whenever the payload shape changes; older snapshots are ignored.
const VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
struct Envelope {
    version: u32,
    #[serde(flatten)]
    payload: Payload,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", content = "items", rename_all = "camelCase")]
enum Payload {
    Databases(Vec<Named>),
    Schemas(Vec<Named>),
    Objects(Vec<Object>),
    Index(Vec<Object>),
}

#[derive(Deserialize, Serialize)]
struct Named {
    name: String,
    system: bool,
    comment: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct Object {
    schema: String,
    name: String,
    kind: String,
    arguments: Option<String>,
    rows: Option<u64>,
    comment: Option<String>,
}

impl From<&ObjectSummary> for Object {
    fn from(object: &ObjectSummary) -> Self {
        Self {
            schema: object.schema.clone(),
            name: object.name.clone(),
            kind: object.kind.as_str().to_owned(),
            arguments: object.arguments.clone(),
            rows: object.row_estimate,
            comment: object.comment.clone(),
        }
    }
}

impl Object {
    fn summary(self) -> Option<ObjectSummary> {
        Some(ObjectSummary {
            kind: ObjectKind::parse(&self.kind)?,
            schema: self.schema,
            name: self.name,
            arguments: self.arguments,
            row_estimate: self.rows,
            comment: self.comment,
        })
    }
}

fn objects(items: &[ObjectSummary]) -> Vec<Object> {
    items.iter().map(Object::from).collect()
}

fn summaries(items: Vec<Object>) -> Option<Vec<ObjectSummary>> {
    items.into_iter().map(Object::summary).collect()
}

/// Encode a listing as a snapshot payload.
pub(crate) fn encode(listing: &Listing) -> Result<Vec<u8>, serde_json::Error> {
    let payload = match listing {
        Listing::Databases(items) => Payload::Databases(
            items
                .iter()
                .map(|item| Named {
                    name: item.name.clone(),
                    system: item.system,
                    comment: item.comment.clone(),
                })
                .collect(),
        ),
        Listing::Schemas(items) => Payload::Schemas(
            items
                .iter()
                .map(|item| Named {
                    name: item.name.clone(),
                    system: item.system,
                    comment: item.comment.clone(),
                })
                .collect(),
        ),
        Listing::Objects(items) => Payload::Objects(objects(items)),
        Listing::Index(index) => Payload::Index(objects(index.objects())),
    };
    serde_json::to_vec(&Envelope {
        version: VERSION,
        payload,
    })
}

/// Decode a snapshot payload; `None` when it is unreadable or outdated.
pub(crate) fn decode(bytes: &[u8]) -> Option<Listing> {
    let envelope: Envelope = serde_json::from_slice(bytes).ok()?;
    if envelope.version != VERSION {
        return None;
    }
    Some(match envelope.payload {
        Payload::Databases(items) => Listing::Databases(
            items
                .into_iter()
                .map(|item| DatabaseSummary {
                    name: item.name,
                    system: item.system,
                    comment: item.comment,
                })
                .collect(),
        ),
        Payload::Schemas(items) => Listing::Schemas(
            items
                .into_iter()
                .map(|item| SchemaSummary {
                    name: item.name,
                    system: item.system,
                    comment: item.comment,
                })
                .collect(),
        ),
        Payload::Objects(items) => Listing::Objects(summaries(items)?),
        Payload::Index(items) => Listing::Index(NameIndex::new(summaries(items)?)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(name: &str, kind: ObjectKind) -> ObjectSummary {
        ObjectSummary {
            schema: "public".to_owned(),
            name: name.to_owned(),
            kind,
            arguments: (kind == ObjectKind::Function).then(|| "integer".to_owned()),
            row_estimate: (kind == ObjectKind::Table).then_some(42),
            comment: Some("note".to_owned()),
        }
    }

    #[test]
    fn listings_round_trip() -> Result<(), serde_json::Error> {
        let items = vec![
            object("orders", ObjectKind::Table),
            object("total", ObjectKind::Function),
        ];
        let decoded = decode(&encode(&Listing::Objects(items.clone()))?);
        assert_eq!(
            decoded.as_ref().and_then(Listing::objects),
            Some(&items[..])
        );
        let index = decode(&encode(&Listing::Index(NameIndex::new(items.clone())))?);
        assert_eq!(index.as_ref().and_then(Listing::objects), Some(&items[..]));
        let schemas = vec![SchemaSummary {
            name: "public".to_owned(),
            system: false,
            comment: None,
        }];
        let decoded = decode(&encode(&Listing::Schemas(schemas.clone()))?);
        assert_eq!(
            decoded.as_ref().and_then(Listing::schemas),
            Some(&schemas[..])
        );
        Ok(())
    }

    #[test]
    fn unreadable_or_outdated_payloads_are_ignored() {
        assert!(decode(b"not json").is_none());
        assert!(decode(br#"{"version":0,"kind":"schemas","items":[]}"#).is_none());
        let unknown_kind = br#"{"version":1,"kind":"objects","items":[
            {"schema":"s","name":"n","kind":"index","arguments":null,"rows":null,"comment":null}]}"#;
        assert!(decode(unknown_kind).is_none());
    }
}
