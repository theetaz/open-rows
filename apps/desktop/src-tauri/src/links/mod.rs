// SPDX-License-Identifier: Apache-2.0
//! `openrows://` links (FR-APP-09, docs/16 §3.1). A link only ever *opens*
//! something: a saved connection, a local file, a schema object or a table
//! with filters. Links never carry secrets or SQL; the webview previews the
//! context and asks before anything connects, and nothing ever runs.
mod params;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use serde::Serialize;
use specta::Type;
use url::Url;

use crate::connections::ObjectKindDto;
use crate::tables::{FilterDto, FilterOperatorDto};
use params::Params;

/// The registered URL scheme (`CFBundleURLTypes` in `Info.plist`).
pub(crate) const SCHEME: &str = "openrows";
/// Longest accepted link; real links stay far below.
const MAX_LINK_BYTES: usize = 8 * 1024;
/// Most filters one table link may carry.
const MAX_FILTERS: usize = 16;
/// File types the app opens, mirroring `bundle.fileAssociations`.
const FILE_EXTENSIONS: &[&str] = &["sql", "sqlite", "sqlite3", "db", "duckdb"];

/// Why a link was rejected. Messages never repeat the link or its values,
/// so they are safe to show and to log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum LinkError {
    #[error("The link is longer than 8 KiB.")]
    TooLong,
    #[error("This is not a valid openrows:// link.")]
    Malformed,
    #[error("Only openrows:// links can be opened.")]
    Scheme,
    #[error("Unknown link action; use connection, file, object or table.")]
    Action,
    #[error("Links never carry passwords, tokens or other secrets.")]
    Secret,
    #[error("The link contains invalid characters.")]
    Encoding,
    #[error("The link has a parameter OpenRows does not know.")]
    Unknown,
    #[error("The link repeats a parameter.")]
    Duplicate,
    #[error("The link is missing the “{0}” parameter.")]
    Missing(&'static str),
    #[error("The link's “{0}” parameter is invalid.")]
    Invalid(&'static str),
    #[error("A link can carry at most 16 filters.")]
    TooManyFilters,
    #[error("Links cannot carry raw SQL filters.")]
    RawFilter,
}

/// The saved connection a link names: its id, its name, or both (the id wins).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LinkConnectionDto {
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
}

/// A table-like relation a link opens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LinkObjectDto {
    pub(crate) name: String,
    pub(crate) kind: ObjectKindDto,
}

/// What a link asks the workspace to open, after the user confirms it.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum OpenLinkDto {
    /// Select (and connect) a saved connection, optionally on one database.
    Connection {
        connection: LinkConnectionDto,
        database: Option<String>,
    },
    /// Show a schema, or open a relation's data view.
    Object {
        connection: LinkConnectionDto,
        database: String,
        schema: String,
        object: Option<LinkObjectDto>,
    },
    /// Open a relation's data view with parameterised filters, all of which must match.
    Table {
        connection: LinkConnectionDto,
        database: String,
        schema: String,
        object: LinkObjectDto,
        filters: Vec<FilterDto>,
    },
}

/// A validated link: something for the workspace, or a local file to hand off.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ParsedLink {
    Open(OpenLinkDto),
    File(PathBuf),
}

/// Parse link text (the CLI's argument).
///
/// # Errors
/// The text is too long, not a URL, or not a valid `openrows://` link.
pub(crate) fn parse(text: &str) -> Result<ParsedLink, LinkError> {
    if text.len() > MAX_LINK_BYTES {
        return Err(LinkError::TooLong);
    }
    let url = Url::parse(text).map_err(|_| LinkError::Malformed)?;
    parse_url(&url)
}

/// Parse a URL the operating system delivered.
///
/// # Errors
/// The URL is not a valid `openrows://` link (see [`LinkError`]).
pub(crate) fn parse_url(url: &Url) -> Result<ParsedLink, LinkError> {
    if url.as_str().len() > MAX_LINK_BYTES {
        return Err(LinkError::TooLong);
    }
    if url.scheme() != SCHEME {
        return Err(LinkError::Scheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(LinkError::Secret);
    }
    if url.port().is_some() || url.fragment().is_some() || !matches!(url.path(), "" | "/") {
        return Err(LinkError::Malformed);
    }
    let mut params = Params::read(url)?;
    let action = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let link = match action.as_str() {
        "connection" => ParsedLink::Open(OpenLinkDto::Connection {
            connection: connection(&mut params)?,
            database: params.name("database")?,
        }),
        "file" => ParsedLink::File(file(&mut params)?),
        "object" => ParsedLink::Open(object(&mut params)?),
        "table" => ParsedLink::Open(table(&mut params)?),
        _ => return Err(LinkError::Action),
    };
    params.finish()?;
    Ok(link)
}

fn connection(params: &mut Params) -> Result<LinkConnectionDto, LinkError> {
    let id = params.name("id")?;
    if let Some(id) = &id {
        crate::connections::profiles::profile_id(id).map_err(|_| LinkError::Invalid("id"))?;
    }
    let name = params.name("name")?;
    if id.is_none() && name.is_none() {
        return Err(LinkError::Missing("name"));
    }
    Ok(LinkConnectionDto { id, name })
}

fn file(params: &mut Params) -> Result<PathBuf, LinkError> {
    let path = PathBuf::from(params.value("path")?.ok_or(LinkError::Missing("path"))?);
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let known = extension.is_some_and(|extension| FILE_EXTENSIONS.contains(&extension.as_str()));
    if !path.is_absolute() || !known {
        return Err(LinkError::Invalid("path"));
    }
    Ok(path)
}

/// The relation named by `object` and `kind` (a table when no kind is given).
fn relation(params: &mut Params) -> Result<Option<LinkObjectDto>, LinkError> {
    let name = params.name("object")?;
    let kind = match params.name("kind")?.as_deref() {
        None | Some("table") => ObjectKindDto::Table,
        Some("view") => ObjectKindDto::View,
        Some("materializedView") => ObjectKindDto::MaterializedView,
        Some("foreignTable") => ObjectKindDto::ForeignTable,
        Some(_) => return Err(LinkError::Invalid("kind")),
    };
    Ok(name.map(|name| LinkObjectDto { name, kind }))
}

fn object(params: &mut Params) -> Result<OpenLinkDto, LinkError> {
    Ok(OpenLinkDto::Object {
        connection: connection(params)?,
        database: params.required("database")?,
        schema: params.required("schema")?,
        object: relation(params)?,
    })
}

fn table(params: &mut Params) -> Result<OpenLinkDto, LinkError> {
    let connection = connection(params)?;
    let database = params.required("database")?;
    let schema = params.required("schema")?;
    let object = relation(params)?.ok_or(LinkError::Missing("object"))?;
    let texts = params.filters();
    if texts.len() > MAX_FILTERS {
        return Err(LinkError::TooManyFilters);
    }
    let filters = texts
        .iter()
        .map(|text| filter(text))
        .collect::<Result<_, _>>()?;
    Ok(OpenLinkDto::Table {
        connection,
        database,
        schema,
        object,
        filters,
    })
}

/// One `filter` value: its own form-encoded `column`, `op`, `value` and `second`.
fn filter(text: &str) -> Result<FilterDto, LinkError> {
    let mut params = Params::nested(text)?;
    let column = params.required("column")?;
    let operator = params.required("op")?;
    let operator: FilterOperatorDto = serde_json::from_value(serde_json::Value::String(operator))
        .map_err(|_| LinkError::Invalid("op"))?;
    if operator == FilterOperatorDto::Raw {
        return Err(LinkError::RawFilter);
    }
    let value = params.value("value")?.unwrap_or_default();
    let second = params.value("second")?.unwrap_or_default();
    params.finish()?;
    Ok(FilterDto {
        column,
        operator,
        value,
        second,
    })
}
