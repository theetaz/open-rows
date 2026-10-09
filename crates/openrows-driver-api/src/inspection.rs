// SPDX-License-Identifier: Apache-2.0
//! Read-only catalog inspection (FR-SCHEMA-07, FR-ADMIN-02).
use crate::catalog::ObjectKind;

/// Identity of one schema object, including its routine overload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectIdentity {
    /// Owning schema.
    pub schema: String,
    /// Object name.
    pub name: String,
    /// Catalog kind.
    pub kind: ObjectKind,
    /// Routine identity arguments, when present.
    pub arguments: Option<String>,
}

/// One catalog attribute; exact text avoids rounding sequence values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    /// Human-readable attribute label.
    pub name: String,
    /// Server value; NULL means unavailable, never a fabricated default.
    pub value: Option<String>,
}

/// A tabular part of a detail viewer, preserving server order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailSection {
    /// Section label, such as Arguments or Enum labels.
    pub name: String,
    /// Column labels in row order.
    pub columns: Vec<String>,
    /// Text cells, with SQL NULL kept distinct from empty text.
    pub rows: Vec<Vec<Option<String>>>,
}

/// Metadata and optional DDL of a schema object.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectDetail {
    /// Attributes, in display order.
    pub attributes: Vec<Attribute>,
    /// Columns, arguments, fields, labels or constraints.
    pub sections: Vec<DetailSection>,
    /// Definition when the server exposes one to the current user.
    pub definition: Option<String>,
}

/// An installed `PostgreSQL` extension and its available version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionSummary {
    /// Installed extension name.
    pub name: String,
    /// Installed version.
    pub version: String,
    /// Default available version, if installed on the server.
    pub available_version: Option<String>,
    /// Installation schema.
    pub schema: String,
    /// Server description, if available.
    pub description: Option<String>,
}

/// A role or user, with a scoped identity for account/grant lookups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleSummary {
    /// Account or principal name.
    pub name: String,
    /// Host (`MySQL`), server or database (SQL Server), or cluster (`PostgreSQL`).
    pub scope: String,
    /// Public account attributes; never password hashes.
    pub attributes: Vec<Attribute>,
    /// Direct memberships, in name order.
    pub memberships: Vec<String>,
}

/// An explicitly granted permission, read without modifying the account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantSummary {
    /// Server/database/schema/table scope.
    pub scope: String,
    /// Quoted object name, when applicable.
    pub object: String,
    /// Permission or role grant as reported by the server.
    pub privilege: String,
    /// GRANT, DENY or GRANT WITH GRANT OPTION.
    pub state: String,
    /// Grantor, when reported by the server.
    pub grantor: Option<String>,
}

/// Pair a catalog row with its known column labels.
#[must_use]
pub fn attributes(labels: &[&str], row: &[Option<String>]) -> Vec<Attribute> {
    labels
        .iter()
        .enumerate()
        .map(|(index, label)| Attribute {
            name: (*label).to_owned(),
            value: row.get(index).cloned().flatten(),
        })
        .collect()
}

/// A named table of catalog rows with fixed column labels.
#[must_use]
pub fn section(name: &str, columns: &[&str], rows: Vec<Vec<Option<String>>>) -> DetailSection {
    DetailSection {
        name: name.to_owned(),
        columns: columns.iter().map(|name| (*name).to_owned()).collect(),
        rows,
    }
}

/// View columns and stored definition from a relation detail read.
///
/// # Errors
/// A missing relation or a relation that is not a view.
pub fn view_detail(detail: crate::schema::TableDetail) -> Result<ObjectDetail, crate::DriverError> {
    let Some(info) = detail.info.filter(|info| {
        matches!(
            info.kind,
            crate::schema::RelationKind::View | crate::schema::RelationKind::MaterializedView
        )
    }) else {
        return Err(crate::DriverError::Server {
            code: "42704".to_owned(),
            message: "View no longer exists or is not visible".to_owned(),
        });
    };
    Ok(ObjectDetail {
        definition: info.definition.or(info.view_definition),
        attributes: Vec::new(),
        sections: vec![section(
            "Columns",
            &["Name", "Type", "Nullable", "Default", "Comment"],
            detail
                .columns
                .into_iter()
                .map(|column| {
                    vec![
                        Some(column.name),
                        Some(column.data_type),
                        Some(column.nullable.to_string()),
                        column.default,
                        column.comment,
                    ]
                })
                .collect(),
        )],
    })
}
