// SPDX-License-Identifier: Apache-2.0
//! Cache scopes and their stable keys (the snapshot rows' `scope` column).

use openrows_driver_api::catalog::ObjectKind;

/// One lazily read listing of a connection database.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Scope {
    /// Databases on the server (read through the home database).
    Databases,
    /// Schemas of the database.
    Schemas,
    /// Objects of some kinds in one schema (one tree section).
    Objects {
        /// Owning schema.
        schema: String,
        /// Listed kinds, deduplicated in [`ObjectKind::ALL`] order.
        kinds: Vec<ObjectKind>,
    },
    /// The bounded name index of every user schema.
    Index,
}

impl Scope {
    /// Objects of `kinds` in `schema`; the kinds' order and repeats do not
    /// matter, so equal sections share one cache entry.
    #[must_use]
    pub fn objects(schema: &str, kinds: &[ObjectKind]) -> Self {
        Self::Objects {
            schema: schema.to_owned(),
            kinds: ObjectKind::ALL
                .into_iter()
                .filter(|kind| kinds.contains(kind))
                .collect(),
        }
    }

    /// Stable key, e.g. `schemas` or `objects:table,view:public`; the schema
    /// comes last, so any schema name keeps keys unique.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Databases => "databases".to_owned(),
            Self::Schemas => "schemas".to_owned(),
            Self::Index => "index".to_owned(),
            Self::Objects { schema, kinds } => {
                let kinds: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
                format!("objects:{}:{schema}", kinds.join(","))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_scopes_ignore_kind_order_and_repeats() {
        let a = Scope::objects("public", &[ObjectKind::View, ObjectKind::Table]);
        let b = Scope::objects(
            "public",
            &[ObjectKind::Table, ObjectKind::View, ObjectKind::Table],
        );
        assert_eq!(a, b);
        assert_eq!(a.key(), "objects:table,view:public");
    }

    #[test]
    fn keys_are_distinct_even_for_odd_schema_names() {
        let odd = Scope::objects("a:b,c", &[ObjectKind::Table]);
        assert_eq!(odd.key(), "objects:table:a:b,c");
        assert_ne!(Scope::Schemas.key(), Scope::Index.key());
        assert_ne!(Scope::Databases.key(), Scope::Schemas.key());
    }
}
