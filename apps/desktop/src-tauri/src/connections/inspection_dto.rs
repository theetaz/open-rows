// SPDX-License-Identifier: Apache-2.0
//! Read-only object viewer contracts (M2-09).
use super::catalog_dto::ObjectKindDto;
use openrows_driver_api::inspection::{
    Attribute, DetailSection, ExtensionSummary, GrantSummary, ObjectDetail, ObjectIdentity,
    RoleSummary,
};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Object kinds and admin catalogs the connected engine can browse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogFeaturesDto {
    pub(crate) kinds: Vec<ObjectKindDto>,
    pub(crate) extensions: bool,
    pub(crate) roles: bool,
}

/// One named public catalog attribute, with NULL preserved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct AttributeDto {
    name: String,
    value: Option<String>,
}
impl From<Attribute> for AttributeDto {
    fn from(value: Attribute) -> Self {
        Self {
            name: value.name,
            value: value.value,
        }
    }
}

/// Ordered rows of columns, arguments, labels, fields or constraints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct DetailSectionDto {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}
impl From<DetailSection> for DetailSectionDto {
    fn from(value: DetailSection) -> Self {
        Self {
            name: value.name,
            columns: value.columns,
            rows: value.rows,
        }
    }
}

/// Public metadata and a definition, when readable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct ObjectDetailDto {
    attributes: Vec<AttributeDto>,
    sections: Vec<DetailSectionDto>,
    definition: Option<String>,
}
impl From<ObjectDetail> for ObjectDetailDto {
    fn from(value: ObjectDetail) -> Self {
        Self {
            attributes: value.attributes.into_iter().map(Into::into).collect(),
            sections: value.sections.into_iter().map(Into::into).collect(),
            definition: value.definition,
        }
    }
}

/// Identity of one object, including the routine overload.
#[derive(Clone, Debug, Deserialize, Type)]
pub(crate) struct ObjectIdentityDto {
    pub(crate) schema: String,
    name: String,
    pub(crate) kind: ObjectKindDto,
    arguments: Option<String>,
}
impl From<ObjectIdentityDto> for ObjectIdentity {
    fn from(value: ObjectIdentityDto) -> Self {
        Self {
            schema: value.schema,
            name: value.name,
            kind: value.kind.into(),
            arguments: value.arguments,
        }
    }
}

/// One installed extension, including the available default version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtensionDto {
    name: String,
    version: String,
    available_version: Option<String>,
    schema: String,
    description: Option<String>,
}
impl From<ExtensionSummary> for ExtensionDto {
    fn from(value: ExtensionSummary) -> Self {
        Self {
            name: value.name,
            version: value.version,
            available_version: value.available_version,
            schema: value.schema,
            description: value.description,
        }
    }
}

/// A scoped role or user, with attributes and direct memberships.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct RoleDto {
    name: String,
    scope: String,
    attributes: Vec<AttributeDto>,
    memberships: Vec<String>,
}
impl From<RoleSummary> for RoleDto {
    fn from(value: RoleSummary) -> Self {
        Self {
            name: value.name,
            scope: value.scope,
            attributes: value.attributes.into_iter().map(Into::into).collect(),
            memberships: value.memberships,
        }
    }
}

/// Identity of a role/user: host, server, database or cluster scope.
#[derive(Clone, Debug, Deserialize, Type)]
pub(crate) struct RoleIdentityDto {
    pub(crate) name: String,
    pub(crate) scope: String,
}

/// One explicit permission, including DENY or the grant option.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct GrantDto {
    scope: String,
    object: String,
    privilege: String,
    state: String,
    grantor: Option<String>,
}
impl From<GrantSummary> for GrantDto {
    fn from(value: GrantSummary) -> Self {
        Self {
            scope: value.scope,
            object: value.object,
            privilege: value.privilege,
            state: value.state,
            grantor: value.grantor,
        }
    }
}
